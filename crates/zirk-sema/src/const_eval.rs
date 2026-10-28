//! Bounded evaluator for pure scalar constant expressions used in type position.
//!
//! The evaluator deliberately accepts an explicit syntax subset. Calls and
//! every AST form not handled below fail closed; const functions have checked
//! scalar signatures and a deliberately bounded expression body.

use crate::constant_value::{CanonicalConst, CanonicalConstError, ConstValue};
use crate::type_budget::{BudgetExceeded, BudgetKind, TypeComputationBudget};
use crate::{FloatWidth, IntWidth};
use std::collections::{HashMap, HashSet};
use unicode_segmentation::UnicodeSegmentation;
use zirk_ast::{BinaryOp, Expr, UnaryOp};
use zirk_diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstEvalErrorKind {
    UnsupportedExpression,
    DisallowedEffect(&'static str),
    TypeMismatch,
    Overflow,
    InvalidLiteral,
    InvalidFloatResult,
    BudgetExceeded(BudgetExceeded),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEvalError {
    pub span: Span,
    pub kind: ConstEvalErrorKind,
}

#[derive(Debug, Clone)]
pub struct ConstFunction {
    pub parameter_names: Vec<String>,
    pub parameter_types: Vec<String>,
    pub return_type: String,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEnumVariant {
    pub enum_identity: String,
    pub variant_identity: String,
    pub payload_types: Vec<String>,
}

pub fn validate_const_function(
    function: &ConstFunction,
    const_function_names: &HashSet<String>,
) -> Result<(), ConstEvalError> {
    let parameters = function.parameter_names.iter().cloned().collect();
    validate_const_expression(&function.body, &parameters, const_function_names)
}

fn validate_const_expression(
    expression: &Expr,
    parameters: &HashSet<String>,
    const_function_names: &HashSet<String>,
) -> Result<(), ConstEvalError> {
    let unsupported =
        || ConstEvalError::at(expression.span(), ConstEvalErrorKind::UnsupportedExpression);
    match expression {
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Duration(_)
        | Expr::Char(_)
        | Expr::Str(_)
        | Expr::Bool(_)
        | Expr::Null(_) => Ok(()),
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_const_expression(element, parameters, const_function_names)?;
            }
            Ok(())
        }
        Expr::Path(path) if parameters.contains(&path.name) => Ok(()),
        Expr::Path(_) => Err(ConstEvalError::at(
            expression.span(),
            ConstEvalErrorKind::DisallowedEffect(
                "mutable or unresolved global reads are not allowed in a const function",
            ),
        )),
        Expr::Unary(unary) => {
            validate_const_expression(&unary.operand, parameters, const_function_names)
        }
        Expr::Binary(binary) => {
            validate_const_expression(&binary.left, parameters, const_function_names)?;
            validate_const_expression(&binary.right, parameters, const_function_names)
        }
        Expr::Interpolated(interpolated) => {
            for part in &interpolated.parts {
                if let zirk_ast::InterpolatedPart::Expr(value) = part {
                    validate_const_expression(value, parameters, const_function_names)?;
                }
            }
            Ok(())
        }
        #[cfg(feature = "timezone")]
        Expr::Field(field) if !field.safe && expression_name(expression) == "TimeZone.UTC" => {
            Ok(())
        }
        #[cfg(not(feature = "timezone"))]
        Expr::Field(field) if !field.safe && expression_name(expression) == "TimeZone.UTC" => {
            return Err(ConstEvalError::at(
                field.span,
                ConstEvalErrorKind::DisallowedEffect(
                    "time-zone evaluation is excluded from the browser interpreter",
                ),
            ));
        }
        Expr::Call(call) => {
            let callee = expression_name(&call.callee);
            if call.args.iter().any(|arg| arg.is_spread)
                || (callee != "Period" && call.args.iter().any(|arg| arg.name.is_some()))
            {
                return Err(unsupported());
            }
            let pure_temporal_method = matches!(
                call.callee.as_ref(),
                Expr::Field(field)
                    if !field.safe && matches!(field.name.name.as_str(),
                        "in_zone" | "offset_at" | "is_dst_at" | "abbreviation_at")
            );
            if !(const_function_names.contains(&callee)
                || is_const_constructor(&callee)
                || pure_temporal_method)
            {
                let kind = if callee.is_empty() {
                    ConstEvalErrorKind::DisallowedEffect(
                        "dynamic or indirect call is not verified pure",
                    )
                } else {
                    classify_call_effect(&callee)
                };
                return Err(ConstEvalError::at(call.span, kind));
            }
            if pure_temporal_method && let Expr::Field(field) = call.callee.as_ref() {
                validate_const_expression(&field.object, parameters, const_function_names)?;
            }
            for arg in &call.args {
                validate_const_expression(&arg.value, parameters, const_function_names)?;
            }
            Ok(())
        }
        _ => Err(unsupported()),
    }
}

fn is_const_constructor(name: &str) -> bool {
    integer_width(name).is_some()
        || matches!(
            name,
            "Date"
                | "Time"
                | "DateTime"
                | "Date.parse"
                | "Time.parse"
                | "DateTime.parse"
                | "Instant.from_unix_seconds"
                | "Instant.from_unix_milliseconds"
                | "Instant.from_unix_nanoseconds"
                | "Instant.parse"
                | "Instant.in_zone"
                | "TimeZone"
                | "TimeZone.parse"
                | "TimeZone.database_version"
                | "Period"
                | "Period.parse"
                | "Period.years"
                | "Period.months"
                | "Period.weeks"
                | "Period.days"
                | "ZonedDateTime.parse"
                | "ZonedDateTime.from_instant"
        )
}

impl ConstEvalError {
    fn at(span: Span, kind: ConstEvalErrorKind) -> Self {
        Self { span, kind }
    }
}

pub fn evaluate(
    expression: &Expr,
    budget: &mut TypeComputationBudget,
) -> Result<CanonicalConst, ConstEvalError> {
    evaluate_with_functions(expression, budget, &HashMap::new())
}

pub fn evaluate_with_functions(
    expression: &Expr,
    budget: &mut TypeComputationBudget,
    functions: &HashMap<String, ConstFunction>,
) -> Result<CanonicalConst, ConstEvalError> {
    evaluate_with_enum_variants(expression, budget, functions, &HashMap::new())
}

pub fn evaluate_with_enum_variants(
    expression: &Expr,
    budget: &mut TypeComputationBudget,
    functions: &HashMap<String, ConstFunction>,
    enum_variants: &HashMap<String, ConstEnumVariant>,
) -> Result<CanonicalConst, ConstEvalError> {
    let value = evaluate_inner(
        expression,
        budget,
        functions,
        enum_variants,
        &HashMap::new(),
    )?;
    value.charge_aggregate_depth(budget).map_err(|error| {
        ConstEvalError::at(expression.span(), ConstEvalErrorKind::BudgetExceeded(error))
    })?;
    let limit = BudgetKind::CanonicalValueBytes.limit();
    let encoded_len = value.encoded_len_limited(limit).ok_or_else(|| {
        ConstEvalError::at(
            expression.span(),
            ConstEvalErrorKind::BudgetExceeded(BudgetExceeded {
                kind: BudgetKind::CanonicalValueBytes,
                limit,
                attempted: limit.saturating_add(1),
            }),
        )
    })?;
    budget
        .charge(BudgetKind::CanonicalValueBytes, encoded_len)
        .map_err(|error| {
            ConstEvalError::at(expression.span(), ConstEvalErrorKind::BudgetExceeded(error))
        })?;
    Ok(value)
}

fn evaluate_inner(
    expression: &Expr,
    budget: &mut TypeComputationBudget,
    functions: &HashMap<String, ConstFunction>,
    enum_variants: &HashMap<String, ConstEnumVariant>,
    locals: &HashMap<String, CanonicalConst>,
) -> Result<CanonicalConst, ConstEvalError> {
    match budget.with_depth(BudgetKind::ConstantCallDepth, |budget| {
        evaluate_inner_body(expression, budget, functions, enum_variants, locals)
    }) {
        Ok(result) => Ok(result),
        Err(crate::type_budget::DepthBudgetError::Exceeded(error)) => Err(ConstEvalError::at(
            expression.span(),
            ConstEvalErrorKind::BudgetExceeded(error),
        )),
        Err(crate::type_budget::DepthBudgetError::Operation(error)) => Err(error),
        Err(crate::type_budget::DepthBudgetError::InvalidDimension(_)) => unreachable!(
            "constant evaluation uses a depth dimension supported by TypeComputationBudget"
        ),
    }
}

fn evaluate_inner_body(
    expression: &Expr,
    budget: &mut TypeComputationBudget,
    functions: &HashMap<String, ConstFunction>,
    enum_variants: &HashMap<String, ConstEnumVariant>,
    locals: &HashMap<String, CanonicalConst>,
) -> Result<CanonicalConst, ConstEvalError> {
    budget
        .charge(BudgetKind::ConstantFuel, 1)
        .map_err(|error| {
            ConstEvalError::at(expression.span(), ConstEvalErrorKind::BudgetExceeded(error))
        })?;

    let result = match expression {
        Expr::Int(literal) => make(
            "Int32",
            ConstValue::integer(IntWidth::I32, literal.value).map_err(|_| {
                ConstEvalError::at(literal.span, ConstEvalErrorKind::InvalidLiteral)
            })?,
        ),
        Expr::Float(literal) => {
            evaluate_float(&literal.text, literal.width.as_deref(), literal.span)?
        }
        Expr::Duration(literal) => make("Duration", ConstValue::Duration(literal.nanos)),
        Expr::Char(literal) => {
            if literal.value.graphemes(true).count() != 1 {
                return Err(ConstEvalError::at(
                    literal.span,
                    ConstEvalErrorKind::InvalidLiteral,
                ));
            }
            make("Char", ConstValue::Char(literal.value.clone()))
        }
        Expr::Str(literal) => make("String", ConstValue::String(literal.value.clone())),
        Expr::Bool(literal) => make("Boolean", ConstValue::Boolean(literal.value)),
        Expr::Null(_) => make("Null", ConstValue::Null),
        Expr::Tuple(tuple) => {
            let mut elements = Vec::with_capacity(tuple.elements.len());
            for element in &tuple.elements {
                elements.push(evaluate_inner(
                    element,
                    budget,
                    functions,
                    enum_variants,
                    locals,
                )?);
            }
            if elements.len() < 2 {
                return Err(ConstEvalError::at(
                    tuple.span,
                    ConstEvalErrorKind::InvalidLiteral,
                ));
            }
            let tuple_identity = format!(
                "Tuple<{}>",
                elements
                    .iter()
                    .map(|element| element.type_identity.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            CanonicalConst {
                type_identity: tuple_identity.clone(),
                value: ConstValue::Tuple {
                    tuple_identity,
                    elements,
                },
            }
        }
        Expr::Object(object) => {
            // A compile-time object literal is an immutable value snapshot,
            // not a reference to a runtime class instance. Keep this form
            // deliberately narrow: spreads and computed keys could import
            // identity or effects that are not represented by CanonicalConst.
            let mut fields = Vec::with_capacity(object.elements.len());
            for element in &object.elements {
                let zirk_ast::ObjectLiteralElement::Field(field) = element else {
                    return Err(ConstEvalError::at(
                        element.span(),
                        ConstEvalErrorKind::UnsupportedExpression,
                    ));
                };
                if field.dynamic_key.is_some()
                    || fields.iter().any(|(name, _)| name == &field.name.name)
                {
                    return Err(ConstEvalError::at(
                        field.span,
                        ConstEvalErrorKind::InvalidLiteral,
                    ));
                }
                let value = evaluate_inner(&field.value, budget, functions, enum_variants, locals)?;
                fields.push((field.name.name.clone(), value));
            }
            // Object shape identity is independent of source key order. Sort
            // before both deriving the shape and serializing its values.
            fields.sort_by(|left, right| left.0.cmp(&right.0));
            let shape = fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", value.type_identity))
                .collect::<Vec<_>>()
                .join(", ");
            let type_identity = format!("Object<{{{shape}}}>");
            CanonicalConst {
                type_identity: type_identity.clone(),
                value: ConstValue::Aggregate {
                    type_identity,
                    fields,
                },
            }
        }
        Expr::Unary(unary) => {
            let operand = evaluate_inner(&unary.operand, budget, functions, enum_variants, locals)?;
            evaluate_unary(unary.op, operand)
                .map_err(|kind| ConstEvalError::at(unary.span, kind))?
        }
        Expr::Binary(binary) => {
            let left = evaluate_inner(&binary.left, budget, functions, enum_variants, locals)?;
            let right = evaluate_inner(&binary.right, budget, functions, enum_variants, locals)?;
            evaluate_binary(binary.op, left, right)
                .map_err(|kind| ConstEvalError::at(binary.op_span, kind))?
        }
        Expr::Variant(variant) => {
            let key = format!("{}.{}", variant.enum_name.name, variant.variant.name);
            enum_constant_value(&key, variant.span, enum_variants)?
        }
        Expr::Field(field) if !field.safe && expression_name(expression) == "TimeZone.UTC" => {
            let identifier = parse_iana_zone("UTC").ok_or_else(|| {
                ConstEvalError::at(field.span, ConstEvalErrorKind::InvalidLiteral)
            })?;
            make(
                "TimeZone",
                ConstValue::TimeZone {
                    identifier,
                    rule_data: iana_tzdb_version().into(),
                },
            )
        }
        Expr::Field(field) if !field.safe => {
            let key = expression_name(expression);
            enum_constant_value(&key, field.span, enum_variants)?
        }
        Expr::Call(call) => evaluate_constructor(call, budget, functions, enum_variants, locals)?,
        Expr::Path(path) => locals.get(&path.name).cloned().ok_or_else(|| {
            ConstEvalError::at(
                expression.span(),
                ConstEvalErrorKind::DisallowedEffect(
                    "mutable or unresolved global reads are not allowed in a constant expression",
                ),
            )
        })?,
        Expr::Interpolated(interpolated) => {
            let mut output = String::new();
            for part in &interpolated.parts {
                match part {
                    zirk_ast::InterpolatedPart::Literal(_) => {}
                    zirk_ast::InterpolatedPart::Expr(part) => {
                        let value = evaluate_inner(part, budget, functions, enum_variants, locals)?;
                        let rendered = display_value(&value).ok_or_else(|| {
                            ConstEvalError::at(part.span(), ConstEvalErrorKind::TypeMismatch)
                        })?;
                        append_string_bounded(&mut output, &rendered, part.span())?;
                    }
                }
                if let zirk_ast::InterpolatedPart::Literal(text) = part {
                    append_string_bounded(&mut output, text, expression.span())?;
                }
            }
            make("String", ConstValue::String(output))
        }
        _ => {
            return Err(ConstEvalError::at(
                expression.span(),
                ConstEvalErrorKind::UnsupportedExpression,
            ));
        }
    };

    Ok(result)
}

fn enum_constant_value(
    key: &str,
    span: Span,
    enum_variants: &HashMap<String, ConstEnumVariant>,
) -> Result<CanonicalConst, ConstEvalError> {
    let Some(spec) = enum_variants.get(key) else {
        return Err(ConstEvalError::at(
            span,
            ConstEvalErrorKind::UnsupportedExpression,
        ));
    };
    if !spec.payload_types.is_empty() {
        return Err(ConstEvalError::at(span, ConstEvalErrorKind::TypeMismatch));
    }
    Ok(CanonicalConst {
        type_identity: spec.enum_identity.clone(),
        value: ConstValue::Enum {
            enum_identity: spec.enum_identity.clone(),
            variant_identity: spec.variant_identity.clone(),
            fields: Vec::new(),
        },
    })
}

fn make(type_identity: &str, value: ConstValue) -> CanonicalConst {
    CanonicalConst {
        type_identity: type_identity.to_string(),
        value,
    }
}

fn evaluate_constructor(
    call: &zirk_ast::CallExpr,
    budget: &mut TypeComputationBudget,
    functions: &HashMap<String, ConstFunction>,
    enum_variants: &HashMap<String, ConstEnumVariant>,
    locals: &HashMap<String, CanonicalConst>,
) -> Result<CanonicalConst, ConstEvalError> {
    let temporal_method_receiver = match call.callee.as_ref() {
        Expr::Field(field)
            if !field.safe
                && matches!(
                    field.name.name.as_str(),
                    "in_zone" | "offset_at" | "is_dst_at" | "abbreviation_at"
                ) =>
        {
            Some((&field.object, field.name.name.as_str()))
        }
        _ => None,
    };
    let constructor = match call.callee.as_ref() {
        Expr::Path(name) => name.name.clone(),
        Expr::Field(field) if !field.safe && field.name.name == "parse" => {
            let Expr::Path(owner) = field.object.as_ref() else {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::UnsupportedExpression,
                ));
            };
            format!("{}.parse", owner.name)
        }
        Expr::Field(field) if !field.safe => match field.name.name.as_str() {
            "in_zone" => "Instant.in_zone".into(),
            "offset_at" => "TimeZone.offset_at".into(),
            "is_dst_at" => "TimeZone.is_dst_at".into(),
            "abbreviation_at" => "TimeZone.abbreviation_at".into(),
            _ => format!("{}.{}", expression_name(&field.object), field.name.name),
        },
        Expr::Field(field) => {
            let name = format!("{}.{}", expression_name(&field.object), field.name.name);
            return Err(ConstEvalError::at(call.span, classify_call_effect(&name)));
        }
        _ => {
            return Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::DisallowedEffect(
                    "dynamic or indirect call is not verified pure",
                ),
            ));
        }
    };
    #[cfg(not(feature = "timezone"))]
    if matches!(
        constructor.as_str(),
        "Instant.in_zone"
            | "TimeZone"
            | "TimeZone.parse"
            | "TimeZone.database_version"
            | "TimeZone.offset_at"
            | "TimeZone.is_dst_at"
            | "TimeZone.abbreviation_at"
            | "ZonedDateTime.parse"
            | "ZonedDateTime.from_instant"
    ) {
        return Err(ConstEvalError::at(
            call.span,
            ConstEvalErrorKind::DisallowedEffect(
                "time-zone evaluation is excluded from the browser interpreter",
            ),
        ));
    }
    if constructor == "Period" {
        let mut components = [0_i32; 4];
        let mut seen = [false; 4];
        let mut positional = 0_usize;
        for argument in &call.args {
            if argument.is_spread {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::UnsupportedExpression,
                ));
            }
            let index = if let Some(name) = &argument.name {
                match name.name.as_str() {
                    "years" => 0,
                    "months" => 1,
                    "weeks" => 2,
                    "days" => 3,
                    _ => {
                        return Err(ConstEvalError::at(
                            call.span,
                            ConstEvalErrorKind::InvalidLiteral,
                        ));
                    }
                }
            } else {
                let index = positional;
                positional += 1;
                if index >= components.len() {
                    return Err(ConstEvalError::at(
                        call.span,
                        ConstEvalErrorKind::TypeMismatch,
                    ));
                }
                index
            };
            if seen[index] {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::InvalidLiteral,
                ));
            }
            let value = evaluate_inner(&argument.value, budget, functions, enum_variants, locals)?;
            components[index] = integer_arg_i128(&value)
                .and_then(|value| i32::try_from(value).ok())
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::Overflow))?;
            seen[index] = true;
        }
        return Ok(make(
            "Period",
            ConstValue::Period {
                years: components[0],
                months: components[1],
                weeks: components[2],
                days: components[3],
            },
        ));
    }

    if call
        .args
        .iter()
        .any(|arg| arg.name.is_some() || arg.is_spread)
    {
        return Err(ConstEvalError::at(
            call.span,
            ConstEvalErrorKind::UnsupportedExpression,
        ));
    }
    let mut values = Vec::with_capacity(call.args.len());
    if let Some((receiver, _)) = temporal_method_receiver {
        values.push(evaluate_inner(
            receiver,
            budget,
            functions,
            enum_variants,
            locals,
        )?);
    }
    for argument in &call.args {
        values.push(evaluate_inner(
            &argument.value,
            budget,
            functions,
            enum_variants,
            locals,
        )?);
    }
    if let Some(function) = functions.get(&constructor) {
        if values.len() != function.parameter_names.len()
            || values
                .iter()
                .zip(&function.parameter_types)
                .any(|(value, ty)| value.type_identity != *ty)
        {
            return Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            ));
        }
        let bindings = function
            .parameter_names
            .iter()
            .cloned()
            .zip(values)
            .collect::<HashMap<_, _>>();
        let value = evaluate_inner(&function.body, budget, functions, enum_variants, &bindings)?;
        if value.type_identity != function.return_type {
            return Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            ));
        }
        return Ok(value);
    }
    if let Some(spec) = enum_variants.get(&constructor) {
        if values.len() != spec.payload_types.len()
            || values
                .iter()
                .zip(&spec.payload_types)
                .any(|(value, ty)| value.type_identity != *ty)
        {
            return Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            ));
        }
        return Ok(CanonicalConst {
            type_identity: spec.enum_identity.clone(),
            value: ConstValue::Enum {
                enum_identity: spec.enum_identity.clone(),
                variant_identity: spec.variant_identity.clone(),
                fields: values,
            },
        });
    }
    if let Some(width) = integer_width(&constructor) {
        let [value] = values.as_slice() else {
            return Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            ));
        };
        let value = cast_integer_constant(width, &value.value)
            .map_err(|kind| ConstEvalError::at(call.span, kind))?;
        return Ok(make(width.name(), value));
    }
    match (constructor.as_str(), values.as_slice()) {
        ("Instant.from_unix_seconds", [value]) => integer_arg_i128(value)
            .and_then(|seconds| seconds.checked_mul(1_000_000_000))
            .map(|nanos| make("Instant", ConstValue::Instant(nanos)))
            .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::Overflow)),
        ("Instant.from_unix_milliseconds", [value]) => integer_arg_i128(value)
            .and_then(|millis| millis.checked_mul(1_000_000))
            .map(|nanos| make("Instant", ConstValue::Instant(nanos)))
            .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::Overflow)),
        ("Instant.from_unix_nanoseconds", [value]) => integer_arg_i128(value)
            .map(|nanos| make("Instant", ConstValue::Instant(nanos)))
            .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::TypeMismatch)),
        ("Instant.parse", [value]) => match &value.value {
            ConstValue::String(text) => chrono::DateTime::parse_from_rfc3339(text)
                .ok()
                .and_then(|value| {
                    i128::from(value.timestamp())
                        .checked_mul(1_000_000_000)?
                        .checked_add(i128::from(value.timestamp_subsec_nanos()))
                })
                .map(|nanos| make("Instant", ConstValue::Instant(nanos)))
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("TimeZone" | "TimeZone.parse", [value]) => match &value.value {
            ConstValue::String(text) => parse_iana_zone(text)
                .map(|identifier| {
                    make(
                        "TimeZone",
                        ConstValue::TimeZone {
                            identifier,
                            rule_data: iana_tzdb_version().to_string(),
                        },
                    )
                })
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("TimeZone.database_version", []) => Ok(make(
            "String",
            ConstValue::String(iana_tzdb_version().to_string()),
        )),
        ("Period.years" | "Period.months" | "Period.weeks" | "Period.days", [value]) => {
            let Some(value) = integer_arg(value).and_then(|v| i32::try_from(v).ok()) else {
                return Err(ConstEvalError::at(call.span, ConstEvalErrorKind::Overflow));
            };
            let mut period = [0_i32; 4];
            let index = match constructor.as_str() {
                "Period.years" => 0,
                "Period.months" => 1,
                "Period.weeks" => 2,
                _ => 3,
            };
            period[index] = value;
            Ok(make(
                "Period",
                ConstValue::Period {
                    years: period[0],
                    months: period[1],
                    weeks: period[2],
                    days: period[3],
                },
            ))
        }
        ("Period", values) if values.len() == 4 => {
            let Some([years, months, weeks, days]) = values
                .iter()
                .map(|value| integer_arg(value).and_then(|v| i32::try_from(v).ok()))
                .collect::<Option<Vec<_>>>()
                .and_then(|values| <[i32; 4]>::try_from(values).ok())
            else {
                return Err(ConstEvalError::at(call.span, ConstEvalErrorKind::Overflow));
            };
            Ok(make(
                "Period",
                ConstValue::Period {
                    years,
                    months,
                    weeks,
                    days,
                },
            ))
        }
        ("Period" | "Period.parse", [value]) => match &value.value {
            ConstValue::String(text) => parse_period(text)
                .map(|value| make("Period", value))
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("ZonedDateTime.from_instant" | "Instant.in_zone", [instant, zone]) => {
            let (
                ConstValue::Instant(instant_nanos),
                ConstValue::TimeZone {
                    identifier,
                    rule_data,
                },
            ) = (&instant.value, &zone.value)
            else {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::TypeMismatch,
                ));
            };
            if !rule_data_matches_current(rule_data) {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::InvalidLiteral,
                ));
            }
            Ok(make(
                "ZonedDateTime",
                ConstValue::ZonedDateTime {
                    instant_nanos: *instant_nanos,
                    identifier: identifier.clone(),
                    rule_data: rule_data.clone(),
                },
            ))
        }
        ("ZonedDateTime.parse", [value]) => match &value.value {
            ConstValue::String(text) => parse_zoned_datetime(text)
                .map(|value| make("ZonedDateTime", value))
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("TimeZone.offset_at", [zone, instant]) => zone_instant_info(zone, instant)
            .map(|(seconds, _, _)| {
                make(
                    "Duration",
                    ConstValue::Duration(i64::from(seconds) * 1_000_000_000),
                )
            })
            .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
        ("TimeZone.is_dst_at", [zone, instant]) => zone_instant_info(zone, instant)
            .map(|(_, is_dst, _)| make("Boolean", ConstValue::Boolean(is_dst)))
            .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
        ("TimeZone.abbreviation_at", [zone, instant]) => zone_instant_info(zone, instant)
            .map(|(_, _, abbreviation)| make("String", ConstValue::String(abbreviation)))
            .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
        ("Date.parse", [value]) => match &value.value {
            ConstValue::String(text) => zirk_runtime_core::temporal::parse_date_text(text)
                .map(|days| make("Date", ConstValue::Date(days)))
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("Time.parse", [value]) => match &value.value {
            ConstValue::String(text) => zirk_runtime_core::temporal::parse_time_text(text)
                .map(|nanos| make("Time", ConstValue::Time(nanos)))
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("DateTime.parse", [value]) => match &value.value {
            ConstValue::String(text) => zirk_runtime_core::temporal::parse_datetime_text(text)
                .map(|value| make("DateTime", ConstValue::DateTime(value)))
                .ok_or_else(|| ConstEvalError::at(call.span, ConstEvalErrorKind::InvalidLiteral)),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("Date", [year, month, day]) => {
            let (year, month, day) = (integer_arg(year), integer_arg(month), integer_arg(day));
            let (Some(year), Some(month), Some(day)) = (year, month, day) else {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::TypeMismatch,
                ));
            };
            if !zirk_runtime_core::temporal::date_is_valid(year, month, day) {
                return Err(ConstEvalError::at(
                    call.span,
                    ConstEvalErrorKind::InvalidLiteral,
                ));
            }
            Ok(make(
                "Date",
                ConstValue::Date(zirk_runtime_core::temporal::days_from_civil(
                    year, month, day,
                )),
            ))
        }
        ("Time", [hour, minute]) => time_value(call.span, hour, minute, None, None),
        ("Time", [hour, minute, second]) => time_value(call.span, hour, minute, Some(second), None),
        ("Time", [hour, minute, second, nanos]) => {
            time_value(call.span, hour, minute, Some(second), Some(nanos))
        }
        ("DateTime", [date, time]) => match (&date.value, &time.value) {
            (ConstValue::Date(days), ConstValue::Time(nanos)) => Ok(make(
                "DateTime",
                ConstValue::DateTime(zirk_runtime_core::temporal::datetime_pack(*days, *nanos)),
            )),
            _ => Err(ConstEvalError::at(
                call.span,
                ConstEvalErrorKind::TypeMismatch,
            )),
        },
        ("DateTime", [year, month, day, hour, minute]) => {
            datetime_components(call.span, year, month, day, hour, minute, None, None)
        }
        ("DateTime", [year, month, day, hour, minute, second]) => datetime_components(
            call.span,
            year,
            month,
            day,
            hour,
            minute,
            Some(second),
            None,
        ),
        ("DateTime", [year, month, day, hour, minute, second, nanos]) => datetime_components(
            call.span,
            year,
            month,
            day,
            hour,
            minute,
            Some(second),
            Some(nanos),
        ),
        _ => Err(ConstEvalError::at(
            call.span,
            classify_call_effect(&constructor),
        )),
    }
}

fn expression_name(expression: &Expr) -> String {
    match expression {
        Expr::Path(path) => path.name.clone(),
        Expr::Field(field) => format!("{}.{}", expression_name(&field.object), field.name.name),
        _ => String::new(),
    }
}

fn parse_iana_zone(value: &str) -> Option<String> {
    #[cfg(feature = "timezone")]
    {
        use std::str::FromStr;
        let mut canonical = chrono_tz::Tz::from_str(value).ok()?.name().to_string();
        let links = include_str!("../data/iana-tzdb-links.txt");
        let (version_line, mappings) = links.split_once('\n')?;
        if !version_line.contains(iana_tzdb_version().as_str()) {
            return None;
        }
        for _ in 0..=mappings.lines().count() {
            let next = mappings.lines().find_map(|line| {
                let (alias, target) = line.split_once('=')?;
                (alias == canonical).then_some(target)
            });
            let Some(next) = next else {
                return Some(canonical);
            };
            canonical = next.to_string();
        }
        None
    }
    #[cfg(not(feature = "timezone"))]
    {
        let _ = value;
        None
    }
}

fn rule_data_matches_current(value: &str) -> bool {
    #[cfg(feature = "timezone")]
    {
        value == iana_tzdb_version()
    }
    #[cfg(not(feature = "timezone"))]
    {
        let _ = value;
        false
    }
}

fn iana_tzdb_version() -> String {
    #[cfg(feature = "timezone")]
    {
        chrono_tz::IANA_TZDB_VERSION.to_string()
    }
    #[cfg(not(feature = "timezone"))]
    {
        "unavailable-in-browser".to_string()
    }
}

fn parse_period(value: &str) -> Option<ConstValue> {
    let mut input = value.strip_prefix('P')?;
    if input.is_empty() {
        return None;
    }
    let mut components = [0_i32; 4];
    let mut seen = [false; 4];
    while !input.is_empty() {
        let digits = input.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let amount = input[..digits].parse::<i32>().ok()?;
        let unit = input.as_bytes().get(digits).copied()?;
        let index = match unit {
            b'Y' => 0,
            b'M' => 1,
            b'W' => 2,
            b'D' => 3,
            _ => return None,
        };
        if seen[index] {
            return None;
        }
        seen[index] = true;
        components[index] = amount;
        input = &input[digits + 1..];
    }
    Some(ConstValue::Period {
        years: components[0],
        months: components[1],
        weeks: components[2],
        days: components[3],
    })
}

fn parse_zoned_datetime(value: &str) -> Option<ConstValue> {
    #[cfg(feature = "timezone")]
    {
        use chrono::{Offset, TimeZone};
        use std::str::FromStr;

        let (timestamp, zone) = value.strip_suffix(']')?.rsplit_once('[')?;
        let identifier = parse_iana_zone(zone)?;
        let zone = chrono_tz::Tz::from_str(zone).ok()?;
        let parsed = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
        let zone_offset = zone
            .offset_from_utc_datetime(&parsed.naive_utc())
            .fix()
            .local_minus_utc();
        if zone_offset != parsed.offset().local_minus_utc() {
            return None;
        }
        let instant_nanos = i128::from(parsed.timestamp())
            .checked_mul(1_000_000_000)?
            .checked_add(i128::from(parsed.timestamp_subsec_nanos()))?;
        Some(ConstValue::ZonedDateTime {
            instant_nanos,
            identifier,
            rule_data: iana_tzdb_version().to_string(),
        })
    }
    #[cfg(not(feature = "timezone"))]
    {
        let _ = value;
        None
    }
}

fn zone_instant_info(
    zone: &CanonicalConst,
    instant: &CanonicalConst,
) -> Option<(i32, bool, String)> {
    #[cfg(feature = "timezone")]
    {
        use chrono::{Offset, TimeZone, Utc};
        use chrono_tz::{OffsetComponents, OffsetName};
        use std::str::FromStr;

        let ConstValue::TimeZone {
            identifier,
            rule_data,
        } = &zone.value
        else {
            return None;
        };
        let ConstValue::Instant(instant_nanos) = &instant.value else {
            return None;
        };
        if !rule_data_matches_current(rule_data) {
            return None;
        }
        let tz = chrono_tz::Tz::from_str(identifier).ok()?;
        let seconds = i64::try_from(instant_nanos.div_euclid(1_000_000_000)).ok()?;
        let nanos = u32::try_from(instant_nanos.rem_euclid(1_000_000_000)).ok()?;
        let instant = chrono::DateTime::<Utc>::from_timestamp(seconds, nanos)?;
        let offset = tz.offset_from_utc_datetime(&instant.naive_utc());
        Some((
            offset.fix().local_minus_utc(),
            offset.dst_offset().num_seconds() != 0,
            offset
                .abbreviation()
                .map(str::to_string)
                .unwrap_or_else(|| format_utc_offset(offset.fix().local_minus_utc())),
        ))
    }
    #[cfg(not(feature = "timezone"))]
    {
        let _ = (zone, instant);
        None
    }
}

#[cfg(feature = "timezone")]
fn format_utc_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let total = seconds.unsigned_abs();
    let hours = total / 3_600;
    let minutes = (total % 3_600) / 60;
    let seconds = total % 60;
    if seconds == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{seconds:02}")
    }
}

fn classify_call_effect(name: &str) -> ConstEvalErrorKind {
    let effect = if name.starts_with("IO.")
        || name.starts_with("Console.")
        || name.starts_with("Stdin.")
        || name.starts_with("Stdout.")
    {
        "I/O is not allowed in a constant expression"
    } else if name.starts_with("Env.") {
        "environment access is not allowed in a constant expression"
    } else if name.starts_with("File.") || name.starts_with("Path.") {
        "filesystem access is not allowed in a constant expression"
    } else if name.starts_with("Http.")
        || name.starts_with("Network.")
        || name.starts_with("Socket.")
    {
        "network access is not allowed in a constant expression"
    } else if name.starts_with("Clock.")
        || name.starts_with("DateTime.now")
        || name.starts_with("Instant.now")
        || name.starts_with("ZonedDateTime.now")
    {
        "clock access is not allowed in a constant expression"
    } else if name.starts_with("TimeZone.system") {
        "host time-zone configuration is not allowed in a constant expression"
    } else if name.starts_with("Random.") {
        "randomness is not allowed in a constant expression"
    } else if name.starts_with("Task.") || name.starts_with("Thread.") {
        "task or thread creation is not allowed in a constant expression"
    } else {
        "call is not declared and verified as a pure constant function"
    };
    ConstEvalErrorKind::DisallowedEffect(effect)
}

fn integer_width(name: &str) -> Option<IntWidth> {
    match name {
        "Int8" => Some(IntWidth::I8),
        "Int16" => Some(IntWidth::I16),
        "Int32" => Some(IntWidth::I32),
        "Int64" => Some(IntWidth::I64),
        "Int128" => Some(IntWidth::I128),
        "UInt8" => Some(IntWidth::U8),
        "UInt16" => Some(IntWidth::U16),
        "UInt32" => Some(IntWidth::U32),
        "UInt64" => Some(IntWidth::U64),
        "UInt128" => Some(IntWidth::U128),
        _ => None,
    }
}

fn cast_integer_constant(
    target: IntWidth,
    value: &ConstValue,
) -> Result<ConstValue, ConstEvalErrorKind> {
    if target.signed() {
        let signed = match value {
            ConstValue::Integer { value, .. } => *value,
            ConstValue::BigUnsigned { value, .. } => {
                i128::try_from(*value).map_err(|_| ConstEvalErrorKind::Overflow)?
            }
            _ => return Err(ConstEvalErrorKind::TypeMismatch),
        };
        ConstValue::integer(target, signed).map_err(map_value_error)
    } else {
        let unsigned = match value {
            ConstValue::Integer { value, .. } => {
                u128::try_from(*value).map_err(|_| ConstEvalErrorKind::Overflow)?
            }
            ConstValue::BigUnsigned { value, .. } => *value,
            _ => return Err(ConstEvalErrorKind::TypeMismatch),
        };
        ConstValue::unsigned_integer(target, unsigned).map_err(map_value_error)
    }
}

fn integer_arg(value: &CanonicalConst) -> Option<i32> {
    match &value.value {
        ConstValue::Integer { value, .. } => i32::try_from(*value).ok(),
        ConstValue::BigUnsigned { value, .. } => i32::try_from(*value).ok(),
        _ => None,
    }
}

fn integer_arg_i128(value: &CanonicalConst) -> Option<i128> {
    match &value.value {
        ConstValue::Integer { value, .. } => Some(*value),
        ConstValue::BigUnsigned { value, .. } => i128::try_from(*value).ok(),
        _ => None,
    }
}

fn is_integer_value(value: &ConstValue) -> bool {
    matches!(
        value,
        ConstValue::Integer { .. } | ConstValue::BigUnsigned { .. }
    )
}

fn integer_mask(bits: u32) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    }
}

fn signed_from_raw(raw: u128, bits: u32) -> i128 {
    let mask = integer_mask(bits);
    let raw = raw & mask;
    if bits == 128 {
        raw as i128
    } else if raw & (1u128 << (bits - 1)) != 0 {
        (raw | !mask) as i128
    } else {
        raw as i128
    }
}

fn integer_from_raw(width: IntWidth, raw: u128) -> Result<ConstValue, CanonicalConstError> {
    let raw = raw & integer_mask(width.bits());
    if width.signed() {
        ConstValue::integer(width, signed_from_raw(raw, width.bits()))
    } else {
        ConstValue::unsigned_integer(width, raw)
    }
}

fn integer_bitwise(
    width: IntWidth,
    left: u128,
    right: i128,
    op: BinaryOp,
) -> Result<ConstValue, ConstEvalErrorKind> {
    let mask = integer_mask(width.bits());
    let left = left & mask;
    let right_bits = right as u128 & mask;
    let result = match op {
        BinaryOp::BitAnd => left & right_bits,
        BinaryOp::BitOr => left | right_bits,
        BinaryOp::BitXor => left ^ right_bits,
        BinaryOp::Shl | BinaryOp::Shr => {
            let shift = u32::try_from(right)
                .ok()
                .filter(|shift| *shift < width.bits())
                .ok_or(ConstEvalErrorKind::InvalidLiteral)?;
            match op {
                BinaryOp::Shl => left.wrapping_shl(shift) & mask,
                BinaryOp::Shr if width.signed() => {
                    (signed_from_raw(left, width.bits()) >> shift) as u128 & mask
                }
                BinaryOp::Shr => left >> shift,
                _ => unreachable!(),
            }
        }
        _ => return Err(ConstEvalErrorKind::UnsupportedExpression),
    };
    integer_from_raw(width, result).map_err(map_value_error)
}

fn time_value(
    span: Span,
    hour: &CanonicalConst,
    minute: &CanonicalConst,
    second: Option<&CanonicalConst>,
    nanos: Option<&CanonicalConst>,
) -> Result<CanonicalConst, ConstEvalError> {
    let (Some(hour), Some(minute)) = (integer_arg(hour), integer_arg(minute)) else {
        return Err(ConstEvalError::at(span, ConstEvalErrorKind::TypeMismatch));
    };
    let second = match second {
        Some(value) => integer_arg(value),
        None => Some(0),
    }
    .ok_or_else(|| ConstEvalError::at(span, ConstEvalErrorKind::TypeMismatch))?;
    let nanos = match nanos {
        Some(value) => integer_arg(value).map(i64::from),
        None => Some(0),
    }
    .ok_or_else(|| ConstEvalError::at(span, ConstEvalErrorKind::TypeMismatch))?;
    if !zirk_runtime_core::temporal::time_is_valid(hour, minute, second, nanos) {
        return Err(ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral));
    }
    Ok(make(
        "Time",
        ConstValue::Time(zirk_runtime_core::temporal::time_nanos(
            hour, minute, second, nanos,
        )),
    ))
}

fn datetime_components(
    span: Span,
    year: &CanonicalConst,
    month: &CanonicalConst,
    day: &CanonicalConst,
    hour: &CanonicalConst,
    minute: &CanonicalConst,
    second: Option<&CanonicalConst>,
    nanos: Option<&CanonicalConst>,
) -> Result<CanonicalConst, ConstEvalError> {
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute)) = (
        integer_arg(year),
        integer_arg(month),
        integer_arg(day),
        integer_arg(hour),
        integer_arg(minute),
    ) else {
        return Err(ConstEvalError::at(span, ConstEvalErrorKind::TypeMismatch));
    };
    if !zirk_runtime_core::temporal::date_is_valid(year, month, day) {
        return Err(ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral));
    }
    let time = time_value(
        span,
        &make(
            "Int32",
            ConstValue::Integer {
                width: IntWidth::I32,
                value: i128::from(hour),
            },
        ),
        &make(
            "Int32",
            ConstValue::Integer {
                width: IntWidth::I32,
                value: i128::from(minute),
            },
        ),
        second,
        nanos,
    )?;
    let ConstValue::Time(time) = time.value else {
        unreachable!()
    };
    let days = zirk_runtime_core::temporal::days_from_civil(year, month, day);
    Ok(make(
        "DateTime",
        ConstValue::DateTime(zirk_runtime_core::temporal::datetime_pack(days, time)),
    ))
}

fn evaluate_unary(
    op: UnaryOp,
    value: CanonicalConst,
) -> Result<CanonicalConst, ConstEvalErrorKind> {
    match (op, value.value) {
        (UnaryOp::Neg, ConstValue::Integer { width, value }) => {
            let value = value.checked_neg().ok_or(ConstEvalErrorKind::Overflow)?;
            Ok(make(
                &value_type_identity(&width),
                ConstValue::integer(width, value).map_err(map_value_error)?,
            ))
        }
        (UnaryOp::BitNot, ConstValue::Integer { width, value }) => {
            let raw = (!(value as u128)) & integer_mask(width.bits());
            let result = integer_from_raw(width, raw).map_err(map_value_error)?;
            Ok(make(&width.name(), result))
        }
        (UnaryOp::BitNot, ConstValue::BigUnsigned { width, value }) => {
            let result = integer_from_raw(width, !value).map_err(map_value_error)?;
            Ok(make(&width.name(), result))
        }
        (UnaryOp::Neg, ConstValue::Decimal { coefficient, scale }) => {
            let coefficient = coefficient
                .checked_neg()
                .ok_or(ConstEvalErrorKind::Overflow)?;
            Ok(make(
                "Decimal",
                ConstValue::decimal(coefficient, scale as u32).map_err(map_value_error)?,
            ))
        }
        (UnaryOp::Neg, ConstValue::Duration(value)) => {
            let value = value.checked_neg().ok_or(ConstEvalErrorKind::Overflow)?;
            Ok(make("Duration", ConstValue::Duration(value)))
        }
        (
            UnaryOp::Neg,
            ConstValue::Period {
                years,
                months,
                weeks,
                days,
            },
        ) => Ok(make(
            "Period",
            ConstValue::Period {
                years: years.checked_neg().ok_or(ConstEvalErrorKind::Overflow)?,
                months: months.checked_neg().ok_or(ConstEvalErrorKind::Overflow)?,
                weeks: weeks.checked_neg().ok_or(ConstEvalErrorKind::Overflow)?,
                days: days.checked_neg().ok_or(ConstEvalErrorKind::Overflow)?,
            },
        )),
        (UnaryOp::Neg, ConstValue::Float { width, bits }) => {
            let sign = match width {
                FloatWidth::F16 => 1u128 << 15,
                FloatWidth::F32 => 1u128 << 31,
                FloatWidth::F64 => 1u128 << 63,
                FloatWidth::F128 => 1u128 << 127,
            };
            let value = ConstValue::float(width, bits ^ sign).map_err(map_value_error)?;
            Ok(make(float_type_identity(width), value))
        }
        (UnaryOp::Not, ConstValue::Boolean(value)) => {
            Ok(make("Boolean", ConstValue::Boolean(!value)))
        }
        _ => Err(ConstEvalErrorKind::TypeMismatch),
    }
}

fn evaluate_binary(
    op: BinaryOp,
    left: CanonicalConst,
    right: CanonicalConst,
) -> Result<CanonicalConst, ConstEvalErrorKind> {
    let decimal_arithmetic = matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::LtEq
            | BinaryOp::Gt
            | BinaryOp::GtEq
    ) && matches!(
        (&left.value, &right.value),
        (
            ConstValue::Decimal { .. }
                | ConstValue::Integer { .. }
                | ConstValue::BigUnsigned { .. },
            ConstValue::Decimal { .. }
                | ConstValue::Integer { .. }
                | ConstValue::BigUnsigned { .. }
        )
    ) && (matches!(left.value, ConstValue::Decimal { .. })
        || matches!(right.value, ConstValue::Decimal { .. })
        || (op == BinaryOp::Div
            && is_integer_value(&left.value)
            && is_integer_value(&right.value)));
    if decimal_arithmetic {
        return evaluate_decimal_binary(op, left.value, right.value);
    }

    if let Some(result) = evaluate_signed_integer_binary(op, &left.value, &right.value) {
        return result;
    }

    if matches!(op, BinaryOp::Shl | BinaryOp::Shr) {
        if let Some(result) = evaluate_integer_shift(op, &left.value, &right.value) {
            return result;
        }
    }

    if let Some(result) = evaluate_unsigned_integer_binary(op, &left.value, &right.value) {
        return result;
    }

    // Widening Float32 to Float64 is exact and is the same common-type choice
    // made by semantic analysis. Numeric literals can adopt the float type of
    // their sibling when the concrete value is exactly representable.
    if matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::LtEq
            | BinaryOp::Gt
            | BinaryOp::GtEq
    ) && matches!(
        (&left.value, &right.value),
        (ConstValue::Float { .. }, ConstValue::Float { .. })
            | (
                ConstValue::Float { .. },
                ConstValue::Integer {
                    width: IntWidth::I32,
                    ..
                }
            )
            | (
                ConstValue::Integer {
                    width: IntWidth::I32,
                    ..
                },
                ConstValue::Float { .. }
            )
    ) && !matches!(
        (&left.value, &right.value),
        (
            ConstValue::Float {
                width: FloatWidth::F16,
                ..
            },
            ConstValue::Float {
                width: FloatWidth::F16,
                ..
            },
        ) | (
            ConstValue::Float {
                width: FloatWidth::F128,
                ..
            },
            ConstValue::Float {
                width: FloatWidth::F128,
                ..
            }
        )
    ) {
        return evaluate_f64_binary(op, left.value, right.value);
    }

    let duration_scalar = matches!(op, BinaryOp::Mul | BinaryOp::Div)
        && matches!(
            (&left.value, &right.value),
            (
                ConstValue::Duration(_),
                ConstValue::Integer { .. } | ConstValue::Float { .. } | ConstValue::Decimal { .. }
            ) | (
                ConstValue::Integer { .. } | ConstValue::Float { .. } | ConstValue::Decimal { .. },
                ConstValue::Duration(_)
            )
        );
    let string_repeat = op == BinaryOp::Mul
        && matches!(
            (&left.value, &right.value),
            (
                ConstValue::String(_),
                ConstValue::Integer { .. } | ConstValue::BigUnsigned { .. }
            ) | (
                ConstValue::Integer { .. } | ConstValue::BigUnsigned { .. },
                ConstValue::String(_)
            )
        );
    let temporal_cross = matches!(
        (&left.value, &right.value, op),
        (
            ConstValue::Date(_),
            ConstValue::Integer { .. },
            BinaryOp::Add | BinaryOp::Sub
        ) | (
            ConstValue::Integer { .. },
            ConstValue::Date(_),
            BinaryOp::Add
        ) | (ConstValue::Date(_), ConstValue::Time(_), BinaryOp::Add)
            | (ConstValue::Time(_), ConstValue::Date(_), BinaryOp::Add)
            | (
                ConstValue::Time(_),
                ConstValue::Duration(_),
                BinaryOp::Add | BinaryOp::Sub
            )
            | (ConstValue::Duration(_), ConstValue::Time(_), BinaryOp::Add)
            | (
                ConstValue::DateTime(_),
                ConstValue::Duration(_),
                BinaryOp::Add | BinaryOp::Sub
            )
            | (
                ConstValue::Instant(_),
                ConstValue::Duration(_),
                BinaryOp::Add | BinaryOp::Sub
            )
            | (
                ConstValue::Duration(_),
                ConstValue::Instant(_),
                BinaryOp::Add
            )
            | (
                ConstValue::ZonedDateTime { .. },
                ConstValue::Duration(_),
                BinaryOp::Add | BinaryOp::Sub
            )
            | (
                ConstValue::Duration(_),
                ConstValue::ZonedDateTime { .. },
                BinaryOp::Add
            )
            | (
                ConstValue::Period { .. },
                ConstValue::Integer { .. },
                BinaryOp::Mul
            )
            | (
                ConstValue::Integer { .. },
                ConstValue::Period { .. },
                BinaryOp::Mul
            )
    );
    if left.type_identity != right.type_identity
        && !duration_scalar
        && !temporal_cross
        && !string_repeat
    {
        return Err(ConstEvalErrorKind::TypeMismatch);
    }
    let type_identity = left.type_identity;
    match (left.value, right.value) {
        (
            ConstValue::Period {
                years: ay,
                months: am,
                weeks: aw,
                days: ad,
            },
            ConstValue::Period {
                years: by,
                months: bm,
                weeks: bw,
                days: bd,
            },
        ) => match op {
            BinaryOp::Add | BinaryOp::Sub => {
                let sign = if op == BinaryOp::Add { 1 } else { -1 };
                Ok(make(
                    "Period",
                    ConstValue::Period {
                        years: ay
                            .checked_add(by.checked_mul(sign).ok_or(ConstEvalErrorKind::Overflow)?)
                            .ok_or(ConstEvalErrorKind::Overflow)?,
                        months: am
                            .checked_add(bm.checked_mul(sign).ok_or(ConstEvalErrorKind::Overflow)?)
                            .ok_or(ConstEvalErrorKind::Overflow)?,
                        weeks: aw
                            .checked_add(bw.checked_mul(sign).ok_or(ConstEvalErrorKind::Overflow)?)
                            .ok_or(ConstEvalErrorKind::Overflow)?,
                        days: ad
                            .checked_add(bd.checked_mul(sign).ok_or(ConstEvalErrorKind::Overflow)?)
                            .ok_or(ConstEvalErrorKind::Overflow)?,
                    },
                ))
            }
            BinaryOp::Eq => Ok(make(
                "Boolean",
                ConstValue::Boolean(ay == by && am == bm && aw == bw && ad == bd),
            )),
            BinaryOp::NotEq => Ok(make(
                "Boolean",
                ConstValue::Boolean(ay != by || am != bm || aw != bw || ad != bd),
            )),
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (
            ConstValue::Period {
                years,
                months,
                weeks,
                days,
            },
            ConstValue::Integer { value, .. },
        )
        | (
            ConstValue::Integer { value, .. },
            ConstValue::Period {
                years,
                months,
                weeks,
                days,
            },
        ) if op == BinaryOp::Mul => {
            let scale = i32::try_from(value).map_err(|_| ConstEvalErrorKind::Overflow)?;
            Ok(make(
                "Period",
                ConstValue::Period {
                    years: years
                        .checked_mul(scale)
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                    months: months
                        .checked_mul(scale)
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                    weeks: weeks
                        .checked_mul(scale)
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                    days: days
                        .checked_mul(scale)
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                },
            ))
        }
        (ConstValue::Instant(instant), ConstValue::Duration(duration)) => match op {
            BinaryOp::Add | BinaryOp::Sub => {
                let duration = i128::from(duration);
                let value = if op == BinaryOp::Add {
                    instant.checked_add(duration)
                } else {
                    instant.checked_sub(duration)
                }
                .ok_or(ConstEvalErrorKind::Overflow)?;
                Ok(make("Instant", ConstValue::Instant(value)))
            }
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (ConstValue::Duration(duration), ConstValue::Instant(instant)) if op == BinaryOp::Add => {
            Ok(make(
                "Instant",
                ConstValue::Instant(
                    instant
                        .checked_add(i128::from(duration))
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                ),
            ))
        }
        (ConstValue::Instant(left), ConstValue::Instant(right)) => match op {
            BinaryOp::Sub => i64::try_from(
                left.checked_sub(right)
                    .ok_or(ConstEvalErrorKind::Overflow)?,
            )
            .map(|difference| make("Duration", ConstValue::Duration(difference)))
            .map_err(|_| ConstEvalErrorKind::Overflow),
            BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(left == right))),
            BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(left != right))),
            BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(left < right))),
            BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(left <= right))),
            BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(left > right))),
            BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(left >= right))),
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (
            ConstValue::ZonedDateTime {
                instant_nanos: instant,
                identifier,
                rule_data,
            },
            ConstValue::Duration(duration),
        ) => {
            if !rule_data_matches_current(&rule_data) {
                return Err(ConstEvalErrorKind::InvalidLiteral);
            }
            let delta = i128::from(duration);
            let instant_nanos = match op {
                BinaryOp::Add => instant.checked_add(delta),
                BinaryOp::Sub => instant.checked_sub(delta),
                _ => return Err(ConstEvalErrorKind::UnsupportedExpression),
            }
            .ok_or(ConstEvalErrorKind::Overflow)?;
            Ok(make(
                "ZonedDateTime",
                ConstValue::ZonedDateTime {
                    instant_nanos,
                    identifier,
                    rule_data,
                },
            ))
        }
        (
            ConstValue::Duration(duration),
            ConstValue::ZonedDateTime {
                instant_nanos,
                identifier,
                rule_data,
            },
        ) if op == BinaryOp::Add => {
            if !rule_data_matches_current(&rule_data) {
                return Err(ConstEvalErrorKind::InvalidLiteral);
            }
            Ok(make(
                "ZonedDateTime",
                ConstValue::ZonedDateTime {
                    instant_nanos: instant_nanos
                        .checked_add(i128::from(duration))
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                    identifier,
                    rule_data,
                },
            ))
        }
        (
            ConstValue::ZonedDateTime {
                instant_nanos: left,
                ..
            },
            ConstValue::ZonedDateTime {
                instant_nanos: right,
                ..
            },
        ) => match op {
            BinaryOp::Sub => i64::try_from(
                left.checked_sub(right)
                    .ok_or(ConstEvalErrorKind::Overflow)?,
            )
            .map(|difference| make("Duration", ConstValue::Duration(difference)))
            .map_err(|_| ConstEvalErrorKind::Overflow),
            BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(left == right))),
            BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(left != right))),
            BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(left < right))),
            BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(left <= right))),
            BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(left > right))),
            BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(left >= right))),
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (
            ConstValue::Integer { width, value: a },
            ConstValue::Integer {
                width: other,
                value: b,
            },
        ) if width == other => {
            if matches!(
                op,
                BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Shl
                    | BinaryOp::Shr
            ) {
                let value = integer_bitwise(width, a as u128, b, op)?;
                return Ok(make(&width.name(), value));
            }
            let value = match op {
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Rem => a.checked_rem(b),
                BinaryOp::Eq => return Ok(make("Boolean", ConstValue::Boolean(a == b))),
                BinaryOp::NotEq => return Ok(make("Boolean", ConstValue::Boolean(a != b))),
                BinaryOp::Lt => return Ok(make("Boolean", ConstValue::Boolean(a < b))),
                BinaryOp::LtEq => return Ok(make("Boolean", ConstValue::Boolean(a <= b))),
                BinaryOp::Gt => return Ok(make("Boolean", ConstValue::Boolean(a > b))),
                BinaryOp::GtEq => return Ok(make("Boolean", ConstValue::Boolean(a >= b))),
                _ => return Err(ConstEvalErrorKind::UnsupportedExpression),
            }
            .ok_or(ConstEvalErrorKind::Overflow)?;
            let value = ConstValue::integer(width, value).map_err(map_value_error)?;
            Ok(make(&type_identity, value))
        }
        (
            ConstValue::Decimal {
                coefficient: a,
                scale: sa,
            },
            ConstValue::Decimal {
                coefficient: b,
                scale: sb,
            },
        ) => match op {
            BinaryOp::Eq => return Ok(make("Boolean", ConstValue::Boolean(a == b && sa == sb))),
            BinaryOp::NotEq => return Ok(make("Boolean", ConstValue::Boolean(a != b || sa != sb))),
            BinaryOp::Add | BinaryOp::Sub => {
                let scale = sa.max(sb);
                let a = scale_decimal(a, sa, scale)?;
                let b = scale_decimal(b, sb, scale)?;
                let coefficient = if op == BinaryOp::Add {
                    a.checked_add(b)
                } else {
                    a.checked_sub(b)
                }
                .ok_or(ConstEvalErrorKind::Overflow)?;
                Ok(make(
                    "Decimal",
                    ConstValue::decimal(coefficient, scale as u32).map_err(map_value_error)?,
                ))
            }
            BinaryOp::Mul => {
                let scale = u32::from(sa) + u32::from(sb);
                let coefficient = a.checked_mul(b).ok_or(ConstEvalErrorKind::Overflow)?;
                Ok(make(
                    "Decimal",
                    ConstValue::decimal(coefficient, scale).map_err(map_value_error)?,
                ))
            }
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (
            ConstValue::Float { width, bits: a },
            ConstValue::Float {
                width: other,
                bits: b,
            },
        ) if width == other => match width {
            FloatWidth::F32 => {
                let a = f32::from_bits(a as u32);
                let b = f32::from_bits(b as u32);
                match op {
                    BinaryOp::Add => float_result(width, f64::from(a + b)),
                    BinaryOp::Sub => float_result(width, f64::from(a - b)),
                    BinaryOp::Mul => float_result(width, f64::from(a * b)),
                    BinaryOp::Div => float_result(width, f64::from(a / b)),
                    BinaryOp::Rem => float_result(width, f64::from(a % b)),
                    BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(a == b))),
                    BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(a != b))),
                    BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(a < b))),
                    BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(a <= b))),
                    BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(a > b))),
                    BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(a >= b))),
                    _ => Err(ConstEvalErrorKind::UnsupportedExpression),
                }
            }
            FloatWidth::F64 => {
                let a = f64::from_bits(a as u64);
                let b = f64::from_bits(b as u64);
                match op {
                    BinaryOp::Add => float_result(width, a + b),
                    BinaryOp::Sub => float_result(width, a - b),
                    BinaryOp::Mul => float_result(width, a * b),
                    BinaryOp::Div => float_result(width, a / b),
                    BinaryOp::Rem => float_result(width, a % b),
                    BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(a == b))),
                    BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(a != b))),
                    BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(a < b))),
                    BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(a <= b))),
                    BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(a > b))),
                    BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(a >= b))),
                    _ => Err(ConstEvalErrorKind::UnsupportedExpression),
                }
            }
            FloatWidth::F16 => {
                let a = zirk_runtime_core::scalar::float16_to_f64(a as u16);
                let b = zirk_runtime_core::scalar::float16_to_f64(b as u16);
                match op {
                    BinaryOp::Add => float_result(width, a + b),
                    BinaryOp::Sub => float_result(width, a - b),
                    BinaryOp::Mul => float_result(width, a * b),
                    BinaryOp::Div => float_result(width, a / b),
                    BinaryOp::Rem => float_result(width, a % b),
                    BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(a == b))),
                    BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(a != b))),
                    BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(a < b))),
                    BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(a <= b))),
                    BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(a > b))),
                    BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(a >= b))),
                    _ => Err(ConstEvalErrorKind::UnsupportedExpression),
                }
            }
            FloatWidth::F128 => {
                let a = zirk_runtime_core::binary_float::Float128::from_bits(a);
                let b = zirk_runtime_core::binary_float::Float128::from_bits(b);
                let operation = match op {
                    BinaryOp::Add => Some(a.add(b)),
                    BinaryOp::Sub => Some(a.sub(b)),
                    BinaryOp::Mul => Some(a.mul(b)),
                    BinaryOp::Div => Some(a.div(b)),
                    BinaryOp::Rem => Some(a.rem(b)),
                    _ => None,
                };
                if let Some(result) = operation {
                    let result = result.map_err(|error| match error {
                        zirk_runtime_core::binary_float::Float128Error::DivisionByZero => {
                            ConstEvalErrorKind::InvalidLiteral
                        }
                        zirk_runtime_core::binary_float::Float128Error::Overflow => {
                            ConstEvalErrorKind::Overflow
                        }
                        _ => ConstEvalErrorKind::InvalidFloatResult,
                    })?;
                    Ok(make(
                        "Float128",
                        ConstValue::Float {
                            width,
                            bits: result.to_bits(),
                        },
                    ))
                } else {
                    let ordering = a
                        .compare(b)
                        .map_err(|_| ConstEvalErrorKind::InvalidFloatResult)?;
                    let equal = ordering == std::cmp::Ordering::Equal;
                    let value = match op {
                        BinaryOp::Eq => Some(equal),
                        BinaryOp::NotEq => Some(!equal),
                        BinaryOp::Lt => Some(ordering == std::cmp::Ordering::Less),
                        BinaryOp::LtEq => Some(ordering != std::cmp::Ordering::Greater),
                        BinaryOp::Gt => Some(ordering == std::cmp::Ordering::Greater),
                        BinaryOp::GtEq => Some(ordering != std::cmp::Ordering::Less),
                        _ => None,
                    };
                    value
                        .map(|value| make("Boolean", ConstValue::Boolean(value)))
                        .ok_or(ConstEvalErrorKind::UnsupportedExpression)
                }
            }
        },
        (ConstValue::Duration(a), ConstValue::Duration(b)) => match op {
            BinaryOp::Add => Ok(make(
                "Duration",
                ConstValue::Duration(a.checked_add(b).ok_or(ConstEvalErrorKind::Overflow)?),
            )),
            BinaryOp::Sub => Ok(make(
                "Duration",
                ConstValue::Duration(a.checked_sub(b).ok_or(ConstEvalErrorKind::Overflow)?),
            )),
            BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(a == b))),
            BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(a != b))),
            BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(a < b))),
            BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(a <= b))),
            BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(a > b))),
            BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(a >= b))),
            BinaryOp::Div if b != 0 => float_result(FloatWidth::F64, a as f64 / b as f64),
            BinaryOp::Div => Err(ConstEvalErrorKind::InvalidLiteral),
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (ConstValue::Date(days), ConstValue::Integer { value, .. }) => {
            let offset = i64::try_from(value).map_err(|_| ConstEvalErrorKind::Overflow)?;
            let result = if op == BinaryOp::Add {
                days.checked_add(offset)
            } else {
                days.checked_sub(offset)
            }
            .ok_or(ConstEvalErrorKind::Overflow)?;
            if zirk_runtime_core::temporal::civil_from_days_checked(result).is_none() {
                return Err(ConstEvalErrorKind::Overflow);
            }
            Ok(make("Date", ConstValue::Date(result)))
        }
        (ConstValue::Integer { value, .. }, ConstValue::Date(days)) if op == BinaryOp::Add => {
            let offset = i64::try_from(value).map_err(|_| ConstEvalErrorKind::Overflow)?;
            let result = days
                .checked_add(offset)
                .ok_or(ConstEvalErrorKind::Overflow)?;
            if zirk_runtime_core::temporal::civil_from_days_checked(result).is_none() {
                return Err(ConstEvalErrorKind::Overflow);
            }
            Ok(make("Date", ConstValue::Date(result)))
        }
        (ConstValue::Date(left), ConstValue::Date(right)) if op == BinaryOp::Sub => {
            let days = left
                .checked_sub(right)
                .ok_or(ConstEvalErrorKind::Overflow)?;
            Ok(make(
                "Int64",
                ConstValue::integer(IntWidth::I64, i128::from(days)).map_err(map_value_error)?,
            ))
        }
        (ConstValue::Date(date), ConstValue::Time(time)) if op == BinaryOp::Add => Ok(make(
            "DateTime",
            ConstValue::DateTime(zirk_runtime_core::temporal::datetime_pack(date, time)),
        )),
        (ConstValue::Time(time), ConstValue::Date(date)) if op == BinaryOp::Add => Ok(make(
            "DateTime",
            ConstValue::DateTime(zirk_runtime_core::temporal::datetime_pack(date, time)),
        )),
        (ConstValue::Time(time), ConstValue::Duration(duration)) => {
            let delta = if op == BinaryOp::Add {
                i128::from(duration)
            } else {
                -i128::from(duration)
            };
            Ok(make(
                "Time",
                ConstValue::Time(
                    (i128::from(time) + delta)
                        .rem_euclid(i128::from(zirk_runtime_core::temporal::NANOS_PER_DAY))
                        as i64,
                ),
            ))
        }
        (ConstValue::Duration(duration), ConstValue::Time(time)) if op == BinaryOp::Add => {
            Ok(make(
                "Time",
                ConstValue::Time(zirk_runtime_core::temporal::time_add_nanos(time, duration)),
            ))
        }
        (ConstValue::Time(left), ConstValue::Time(right)) if op == BinaryOp::Sub => Ok(make(
            "Duration",
            ConstValue::Duration(zirk_runtime_core::temporal::time_diff(left, right)),
        )),
        (ConstValue::DateTime(datetime), ConstValue::Duration(duration)) => {
            let delta = i128::from(duration);
            let result = if op == BinaryOp::Add {
                datetime.checked_add(delta)
            } else {
                datetime.checked_sub(delta)
            }
            .ok_or(ConstEvalErrorKind::Overflow)?;
            if zirk_runtime_core::temporal::datetime_days_checked(result).is_none() {
                return Err(ConstEvalErrorKind::Overflow);
            }
            Ok(make("DateTime", ConstValue::DateTime(result)))
        }
        (ConstValue::Duration(duration), ConstValue::Integer { value, .. }) => {
            let multiplier = i64::try_from(value).map_err(|_| ConstEvalErrorKind::Overflow)?;
            let result = match op {
                BinaryOp::Mul => duration.checked_mul(multiplier),
                BinaryOp::Div if multiplier != 0 => duration.checked_div(multiplier),
                BinaryOp::Div => return Err(ConstEvalErrorKind::InvalidLiteral),
                _ => return Err(ConstEvalErrorKind::UnsupportedExpression),
            }
            .ok_or(ConstEvalErrorKind::Overflow)?;
            Ok(make("Duration", ConstValue::Duration(result)))
        }
        (ConstValue::Integer { value, .. }, ConstValue::Duration(duration))
            if op == BinaryOp::Mul =>
        {
            let multiplier = i64::try_from(value).map_err(|_| ConstEvalErrorKind::Overflow)?;
            Ok(make(
                "Duration",
                ConstValue::Duration(
                    duration
                        .checked_mul(multiplier)
                        .ok_or(ConstEvalErrorKind::Overflow)?,
                ),
            ))
        }
        (ConstValue::Duration(duration), scalar) => {
            let scalar = numeric_to_f64(&scalar).ok_or(ConstEvalErrorKind::TypeMismatch)?;
            let result = if op == BinaryOp::Mul {
                zirk_runtime_core::duration::multiply(duration, scalar)
            } else if op == BinaryOp::Div {
                zirk_runtime_core::duration::divide(duration, scalar)
            } else {
                return Err(ConstEvalErrorKind::UnsupportedExpression);
            }
            .map_err(duration_error)?;
            Ok(make("Duration", ConstValue::Duration(result)))
        }
        (scalar, ConstValue::Duration(duration)) if op == BinaryOp::Mul => {
            let scalar = numeric_to_f64(&scalar).ok_or(ConstEvalErrorKind::TypeMismatch)?;
            let result =
                zirk_runtime_core::duration::multiply(duration, scalar).map_err(duration_error)?;
            Ok(make("Duration", ConstValue::Duration(result)))
        }
        (ConstValue::Boolean(a), ConstValue::Boolean(b)) => match op {
            BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(a == b))),
            BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(a != b))),
            BinaryOp::And => Ok(make("Boolean", ConstValue::Boolean(a && b))),
            BinaryOp::Or => Ok(make("Boolean", ConstValue::Boolean(a || b))),
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (ConstValue::String(a), ConstValue::String(b)) => match op {
            BinaryOp::Add => {
                let length = a.len().saturating_add(b.len());
                ensure_string_size(length as u64)?;
                Ok(make("String", ConstValue::String(a + &b)))
            }
            BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(a == b))),
            BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(a != b))),
            _ => Err(ConstEvalErrorKind::UnsupportedExpression),
        },
        (ConstValue::String(text), ConstValue::Integer { value, .. }) if op == BinaryOp::Mul => {
            let count = u128::try_from(value).map_err(|_| ConstEvalErrorKind::InvalidLiteral)?;
            repeat_string(text, count)
        }
        (ConstValue::String(text), ConstValue::BigUnsigned { value, .. })
            if op == BinaryOp::Mul =>
        {
            repeat_string(text, value)
        }
        (ConstValue::Integer { value, .. }, ConstValue::String(text)) if op == BinaryOp::Mul => {
            let count = u128::try_from(value).map_err(|_| ConstEvalErrorKind::InvalidLiteral)?;
            repeat_string(text, count)
        }
        (ConstValue::BigUnsigned { value, .. }, ConstValue::String(text))
            if op == BinaryOp::Mul =>
        {
            repeat_string(text, value)
        }
        (a, b) if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) => {
            let equal = a == b;
            Ok(make(
                "Boolean",
                ConstValue::Boolean(if op == BinaryOp::Eq { equal } else { !equal }),
            ))
        }
        _ => Err(ConstEvalErrorKind::TypeMismatch),
    }
}

fn repeat_string(text: String, count: u128) -> Result<CanonicalConst, ConstEvalErrorKind> {
    if text.is_empty() || count == 0 {
        return Ok(make("String", ConstValue::String(String::new())));
    }
    let length = (text.len() as u128)
        .checked_mul(count)
        .ok_or_else(|| string_size_exceeded(u64::MAX))?;
    let limit = BudgetKind::CanonicalValueBytes.limit() as u128;
    if length > limit {
        return Err(string_size_exceeded(
            u64::try_from(length).unwrap_or(u64::MAX),
        ));
    }
    let count = usize::try_from(count).map_err(|_| string_size_exceeded(u64::MAX))?;
    Ok(make("String", ConstValue::String(text.repeat(count))))
}

fn string_size_exceeded(attempted: u64) -> ConstEvalErrorKind {
    ConstEvalErrorKind::BudgetExceeded(BudgetExceeded {
        kind: BudgetKind::CanonicalValueBytes,
        limit: BudgetKind::CanonicalValueBytes.limit(),
        attempted,
    })
}

fn ensure_string_size(length: u64) -> Result<(), ConstEvalErrorKind> {
    if length > BudgetKind::CanonicalValueBytes.limit() {
        Err(string_size_exceeded(length))
    } else {
        Ok(())
    }
}

fn append_string_bounded(
    output: &mut String,
    value: &str,
    span: Span,
) -> Result<(), ConstEvalError> {
    let length = output.len().saturating_add(value.len());
    ensure_string_size(length as u64).map_err(|kind| ConstEvalError::at(span, kind))?;
    output.push_str(value);
    Ok(())
}

fn evaluate_decimal_binary(
    op: BinaryOp,
    left: ConstValue,
    right: ConstValue,
) -> Result<CanonicalConst, ConstEvalErrorKind> {
    use zirk_runtime_core::decimal::{self, Decimal, DecimalError};

    let to_decimal = |value| match value {
        ConstValue::Integer { value, .. } => Ok(Decimal::new(value, 0)),
        ConstValue::BigUnsigned { value, .. } => Ok(Decimal::new(
            i128::try_from(value).map_err(|_| ConstEvalErrorKind::Overflow)?,
            0,
        )),
        ConstValue::Decimal { coefficient, scale } => {
            Ok(Decimal::new(coefficient, u32::from(scale)))
        }
        _ => Err(ConstEvalErrorKind::TypeMismatch),
    };
    let left = to_decimal(left)?;
    let right = to_decimal(right)?;
    let result = match op {
        BinaryOp::Add => decimal::add(left, right).map(DecimalResult::Value),
        BinaryOp::Sub => decimal::sub(left, right).map(DecimalResult::Value),
        BinaryOp::Mul => decimal::mul(left, right).map(DecimalResult::Value),
        BinaryOp::Div => decimal::div(left, right).map(DecimalResult::Value),
        BinaryOp::Rem => decimal::rem(left, right).map(DecimalResult::Value),
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq => decimal::compare(left, right).map(|ordering| {
            DecimalResult::Boolean(match op {
                BinaryOp::Eq => ordering == 0,
                BinaryOp::NotEq => ordering != 0,
                BinaryOp::Lt => ordering < 0,
                BinaryOp::LtEq => ordering <= 0,
                BinaryOp::Gt => ordering > 0,
                BinaryOp::GtEq => ordering >= 0,
                _ => unreachable!(),
            })
        }),
        _ => return Err(ConstEvalErrorKind::UnsupportedExpression),
    };
    match result {
        Ok(DecimalResult::Boolean(value)) => Ok(make("Boolean", ConstValue::Boolean(value))),
        Ok(DecimalResult::Value(value)) => Ok(make(
            "Decimal",
            ConstValue::decimal(value.coef, u32::from(value.scale)).map_err(map_value_error)?,
        )),
        Err(DecimalError::DivisionByZero) => Err(ConstEvalErrorKind::InvalidLiteral),
        Err(DecimalError::Overflow | DecimalError::Domain | DecimalError::InvalidCast) => {
            Err(ConstEvalErrorKind::Overflow)
        }
    }
}

fn evaluate_unsigned_integer_binary(
    op: BinaryOp,
    left: &ConstValue,
    right: &ConstValue,
) -> Option<Result<CanonicalConst, ConstEvalErrorKind>> {
    let (left_width, left_value) = unsigned_integer_parts(left)?;
    let (right_width, right_value) = unsigned_integer_parts(right)?;
    let width = common_integer_width(left_width, right_width)?;

    if matches!(
        op,
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr
    ) {
        let right = match i128::try_from(right_value) {
            Ok(value) => value,
            Err(_) => return Some(Err(ConstEvalErrorKind::InvalidLiteral)),
        };
        return Some(
            integer_bitwise(width, left_value, right, op).map(|value| make(&width.name(), value)),
        );
    }

    let result = match op {
        BinaryOp::Add => left_value
            .checked_add(right_value)
            .map(|value| ConstValue::unsigned_integer(width, value).map_err(map_value_error)),
        BinaryOp::Sub => left_value
            .checked_sub(right_value)
            .map(|value| ConstValue::unsigned_integer(width, value).map_err(map_value_error)),
        BinaryOp::Mul => left_value
            .checked_mul(right_value)
            .map(|value| ConstValue::unsigned_integer(width, value).map_err(map_value_error)),
        BinaryOp::Rem if right_value != 0 => Some(
            ConstValue::unsigned_integer(width, left_value % right_value).map_err(map_value_error),
        ),
        BinaryOp::Rem => return Some(Err(ConstEvalErrorKind::InvalidLiteral)),
        BinaryOp::Eq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value == right_value),
            )));
        }
        BinaryOp::NotEq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value != right_value),
            )));
        }
        BinaryOp::Lt => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value < right_value),
            )));
        }
        BinaryOp::LtEq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value <= right_value),
            )));
        }
        BinaryOp::Gt => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value > right_value),
            )));
        }
        BinaryOp::GtEq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value >= right_value),
            )));
        }
        _ => return None,
    };
    Some(match result {
        Some(Ok(value)) => Ok(make(&width.name(), value)),
        Some(Err(error)) => Err(error),
        None => Err(ConstEvalErrorKind::Overflow),
    })
}

fn unsigned_integer_parts(value: &ConstValue) -> Option<(IntWidth, u128)> {
    match value {
        ConstValue::Integer { width, value } if !width.signed() => {
            Some((*width, u128::try_from(*value).ok()?))
        }
        ConstValue::BigUnsigned { width, value } if !width.signed() => Some((*width, *value)),
        _ => None,
    }
}

fn common_integer_width(left: IntWidth, right: IntWidth) -> Option<IntWidth> {
    if left.signed() != right.signed() {
        return None;
    }
    let candidates = if left.signed() {
        [
            IntWidth::I8,
            IntWidth::I16,
            IntWidth::I32,
            IntWidth::I64,
            IntWidth::I128,
        ]
    } else {
        [
            IntWidth::U8,
            IntWidth::U16,
            IntWidth::U32,
            IntWidth::U64,
            IntWidth::U128,
        ]
    };
    candidates.into_iter().find(|candidate| {
        candidate.can_represent_all_of(left) && candidate.can_represent_all_of(right)
    })
}

fn evaluate_signed_integer_binary(
    op: BinaryOp,
    left: &ConstValue,
    right: &ConstValue,
) -> Option<Result<CanonicalConst, ConstEvalErrorKind>> {
    let (left_width, left_value) = match left {
        ConstValue::Integer { width, value } if width.signed() => (*width, *value),
        _ => return None,
    };
    let (right_width, right_value) = match right {
        ConstValue::Integer { width, value } if width.signed() => (*width, *value),
        _ => return None,
    };
    let width = common_integer_width(left_width, right_width)?;

    if matches!(op, BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor) {
        return Some(
            integer_bitwise(width, left_value as u128, right_value, op)
                .map(|value| make(&width.name(), value)),
        );
    }

    let result = match op {
        BinaryOp::Add => left_value
            .checked_add(right_value)
            .map(|value| ConstValue::integer(width, value).map_err(map_value_error)),
        BinaryOp::Sub => left_value
            .checked_sub(right_value)
            .map(|value| ConstValue::integer(width, value).map_err(map_value_error)),
        BinaryOp::Mul => left_value
            .checked_mul(right_value)
            .map(|value| ConstValue::integer(width, value).map_err(map_value_error)),
        BinaryOp::Rem if right_value != 0 => Some(
            left_value
                .checked_rem(right_value)
                .ok_or(ConstEvalErrorKind::Overflow)
                .and_then(|value| ConstValue::integer(width, value).map_err(map_value_error)),
        ),
        BinaryOp::Rem => return Some(Err(ConstEvalErrorKind::InvalidLiteral)),
        BinaryOp::Eq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value == right_value),
            )));
        }
        BinaryOp::NotEq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value != right_value),
            )));
        }
        BinaryOp::Lt => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value < right_value),
            )));
        }
        BinaryOp::LtEq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value <= right_value),
            )));
        }
        BinaryOp::Gt => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value > right_value),
            )));
        }
        BinaryOp::GtEq => {
            return Some(Ok(make(
                "Boolean",
                ConstValue::Boolean(left_value >= right_value),
            )));
        }
        _ => return None,
    };
    Some(match result {
        Some(Ok(value)) => Ok(make(width.name(), value)),
        Some(Err(error)) => Err(error),
        None => Err(ConstEvalErrorKind::Overflow),
    })
}

fn evaluate_integer_shift(
    op: BinaryOp,
    left: &ConstValue,
    right: &ConstValue,
) -> Option<Result<CanonicalConst, ConstEvalErrorKind>> {
    let (width, raw) = match left {
        ConstValue::Integer { width, value } => (*width, *value as u128),
        ConstValue::BigUnsigned { width, value } => (*width, *value),
        _ => return None,
    };
    let shift = match right {
        ConstValue::Integer { value, .. } => u32::try_from(*value),
        ConstValue::BigUnsigned { value, .. } => u32::try_from(*value),
        _ => return None,
    };
    let shift = match shift {
        Ok(value) => i128::from(value),
        Err(_) => return Some(Err(ConstEvalErrorKind::InvalidLiteral)),
    };
    Some(integer_bitwise(width, raw, shift, op).map(|value| make(width.name(), value)))
}

enum DecimalResult {
    Value(zirk_runtime_core::decimal::Decimal),
    Boolean(bool),
}

fn evaluate_f64_binary(
    op: BinaryOp,
    left: ConstValue,
    right: ConstValue,
) -> Result<CanonicalConst, ConstEvalErrorKind> {
    if matches!(
        (&left, &right),
        (
            ConstValue::Float {
                width: FloatWidth::F128,
                ..
            },
            _
        ) | (
            _,
            ConstValue::Float {
                width: FloatWidth::F128,
                ..
            }
        )
    ) {
        let to_f128 = |value| match value {
            ConstValue::Float {
                width: FloatWidth::F128,
                bits,
            } => Ok(zirk_runtime_core::binary_float::Float128::from_bits(bits)),
            ConstValue::Float {
                width: FloatWidth::F16,
                bits,
            } => zirk_runtime_core::binary_float::Float128::from_f64(
                zirk_runtime_core::scalar::float16_to_f64(bits as u16),
            ),
            ConstValue::Float {
                width: FloatWidth::F32,
                bits,
            } => zirk_runtime_core::binary_float::Float128::from_f64(f64::from(f32::from_bits(
                bits as u32,
            ))),
            ConstValue::Float {
                width: FloatWidth::F64,
                bits,
            } => zirk_runtime_core::binary_float::Float128::from_f64(f64::from_bits(bits as u64)),
            ConstValue::Integer {
                width: IntWidth::I32,
                value,
            } => Ok(zirk_runtime_core::binary_float::Float128::from_i128(value)),
            _ => Err(zirk_runtime_core::binary_float::Float128Error::InvalidLiteral),
        };
        let left = to_f128(left).map_err(|_| ConstEvalErrorKind::TypeMismatch)?;
        let right = to_f128(right).map_err(|_| ConstEvalErrorKind::TypeMismatch)?;
        let operation = match op {
            BinaryOp::Add => Some(left.add(right)),
            BinaryOp::Sub => Some(left.sub(right)),
            BinaryOp::Mul => Some(left.mul(right)),
            BinaryOp::Div => Some(left.div(right)),
            BinaryOp::Rem => Some(left.rem(right)),
            _ => None,
        };
        if let Some(result) = operation {
            let result = result.map_err(|error| match error {
                zirk_runtime_core::binary_float::Float128Error::DivisionByZero => {
                    ConstEvalErrorKind::InvalidLiteral
                }
                zirk_runtime_core::binary_float::Float128Error::Overflow => {
                    ConstEvalErrorKind::Overflow
                }
                _ => ConstEvalErrorKind::InvalidFloatResult,
            })?;
            return Ok(make(
                "Float128",
                ConstValue::Float {
                    width: FloatWidth::F128,
                    bits: result.to_bits(),
                },
            ));
        }
        let ordering = left
            .compare(right)
            .map_err(|_| ConstEvalErrorKind::InvalidFloatResult)?;
        let value = match op {
            BinaryOp::Eq => Some(ordering == std::cmp::Ordering::Equal),
            BinaryOp::NotEq => Some(ordering != std::cmp::Ordering::Equal),
            BinaryOp::Lt => Some(ordering == std::cmp::Ordering::Less),
            BinaryOp::LtEq => Some(ordering != std::cmp::Ordering::Greater),
            BinaryOp::Gt => Some(ordering == std::cmp::Ordering::Greater),
            BinaryOp::GtEq => Some(ordering != std::cmp::Ordering::Less),
            _ => None,
        };
        return value
            .map(|value| make("Boolean", ConstValue::Boolean(value)))
            .ok_or(ConstEvalErrorKind::UnsupportedExpression);
    }
    let target = match (&left, &right) {
        (
            ConstValue::Float {
                width: FloatWidth::F64,
                ..
            },
            _,
        )
        | (
            _,
            ConstValue::Float {
                width: FloatWidth::F64,
                ..
            },
        ) => FloatWidth::F64,
        (
            ConstValue::Float {
                width: FloatWidth::F32,
                ..
            },
            _,
        )
        | (
            _,
            ConstValue::Float {
                width: FloatWidth::F32,
                ..
            },
        ) => FloatWidth::F32,
        (
            ConstValue::Float {
                width: FloatWidth::F16,
                ..
            },
            _,
        )
        | (
            _,
            ConstValue::Float {
                width: FloatWidth::F16,
                ..
            },
        ) => FloatWidth::F16,
        _ => return Err(ConstEvalErrorKind::TypeMismatch),
    };
    let to_f64 = |value| match (target, value) {
        (
            FloatWidth::F32,
            ConstValue::Float {
                width: FloatWidth::F32,
                bits,
            },
        ) => Ok(f64::from(f32::from_bits(bits as u32))),
        (
            FloatWidth::F32 | FloatWidth::F64,
            ConstValue::Float {
                width: FloatWidth::F16,
                bits,
            },
        ) => Ok(zirk_runtime_core::scalar::float16_to_f64(bits as u16)),
        (
            FloatWidth::F64,
            ConstValue::Float {
                width: FloatWidth::F32,
                bits,
            },
        ) => Ok(f64::from(f32::from_bits(bits as u32))),
        (
            FloatWidth::F64,
            ConstValue::Float {
                width: FloatWidth::F64,
                bits,
            },
        ) => Ok(f64::from_bits(bits as u64)),
        (
            FloatWidth::F32,
            ConstValue::Integer {
                width: IntWidth::I32,
                value,
            },
        ) if (value as f32) as f64 == value as f64 => Ok(value as f64),
        (
            FloatWidth::F16,
            ConstValue::Integer {
                width: IntWidth::I32,
                value,
            },
        ) if zirk_runtime_core::scalar::float16_to_f64(
            zirk_runtime_core::scalar::float16_bits(value as f64),
        ) == value as f64 =>
        {
            Ok(value as f64)
        }
        (
            FloatWidth::F64,
            ConstValue::Integer {
                width: IntWidth::I32,
                value,
            },
        ) => Ok(value as f64),
        _ => Err(ConstEvalErrorKind::TypeMismatch),
    };
    let left = to_f64(left)?;
    let right = to_f64(right)?;
    let calculate = |value: f64| match target {
        FloatWidth::F16 => float_result(FloatWidth::F16, value),
        FloatWidth::F32 => float_result(FloatWidth::F32, f64::from(value as f32)),
        FloatWidth::F64 => float_result(FloatWidth::F64, value),
        FloatWidth::F128 => Err(ConstEvalErrorKind::UnsupportedExpression),
    };
    match op {
        BinaryOp::Add => calculate(left + right),
        BinaryOp::Sub => calculate(left - right),
        BinaryOp::Mul => calculate(left * right),
        BinaryOp::Div => calculate(left / right),
        BinaryOp::Rem => calculate(left % right),
        BinaryOp::Eq => Ok(make("Boolean", ConstValue::Boolean(left == right))),
        BinaryOp::NotEq => Ok(make("Boolean", ConstValue::Boolean(left != right))),
        BinaryOp::Lt => Ok(make("Boolean", ConstValue::Boolean(left < right))),
        BinaryOp::LtEq => Ok(make("Boolean", ConstValue::Boolean(left <= right))),
        BinaryOp::Gt => Ok(make("Boolean", ConstValue::Boolean(left > right))),
        BinaryOp::GtEq => Ok(make("Boolean", ConstValue::Boolean(left >= right))),
        _ => Err(ConstEvalErrorKind::UnsupportedExpression),
    }
}

fn scale_decimal(coefficient: i128, from: u8, to: u8) -> Result<i128, ConstEvalErrorKind> {
    let mut value = coefficient;
    for _ in from..to {
        value = value.checked_mul(10).ok_or(ConstEvalErrorKind::Overflow)?;
    }
    Ok(value)
}

fn float_result(width: FloatWidth, value: f64) -> Result<CanonicalConst, ConstEvalErrorKind> {
    let bits = match width {
        FloatWidth::F16 => u128::from(zirk_runtime_core::scalar::float16_bits(value)),
        FloatWidth::F32 => u128::from((value as f32).to_bits()),
        FloatWidth::F64 => u128::from(value.to_bits()),
        FloatWidth::F128 => {
            return Err(ConstEvalErrorKind::UnsupportedExpression);
        }
    };
    let value =
        ConstValue::float(width, bits).map_err(|_| ConstEvalErrorKind::InvalidFloatResult)?;
    Ok(make(float_type_identity(width), value))
}

fn numeric_to_f64(value: &ConstValue) -> Option<f64> {
    match value {
        ConstValue::Integer { value, .. } => Some(*value as f64),
        ConstValue::BigUnsigned { value, .. } => Some(*value as f64),
        ConstValue::Decimal { coefficient, scale } => {
            Some(*coefficient as f64 / 10_f64.powi(i32::from(*scale)))
        }
        ConstValue::Float {
            width: FloatWidth::F32,
            bits,
        } => Some(f64::from(f32::from_bits(*bits as u32))),
        ConstValue::Float {
            width: FloatWidth::F64,
            bits,
        } => Some(f64::from_bits(*bits as u64)),
        ConstValue::Float {
            width: FloatWidth::F16,
            bits,
        } => Some(zirk_runtime_core::scalar::float16_to_f64(*bits as u16)),
        _ => None,
    }
}

fn duration_error(error: zirk_runtime_core::duration::ScalarError) -> ConstEvalErrorKind {
    match error {
        zirk_runtime_core::duration::ScalarError::DivisionByZero => {
            ConstEvalErrorKind::InvalidLiteral
        }
        zirk_runtime_core::duration::ScalarError::NonFiniteResult => {
            ConstEvalErrorKind::InvalidFloatResult
        }
        zirk_runtime_core::duration::ScalarError::Overflow => ConstEvalErrorKind::Overflow,
    }
}

fn map_value_error(error: CanonicalConstError) -> ConstEvalErrorKind {
    match error {
        CanonicalConstError::IntegerOutOfRange
        | CanonicalConstError::DecimalCoefficientOutOfRange
        | CanonicalConstError::DecimalScaleOutOfRange => ConstEvalErrorKind::Overflow,
        _ => ConstEvalErrorKind::InvalidLiteral,
    }
}

fn value_type_identity(width: &IntWidth) -> String {
    width.name().to_string()
}
fn float_type_identity(width: FloatWidth) -> &'static str {
    match width {
        FloatWidth::F16 => "Float16",
        FloatWidth::F32 => "Float32",
        FloatWidth::F64 => "Float64",
        FloatWidth::F128 => "Float128",
    }
}

fn evaluate_float(
    text: &str,
    suffix: Option<&str>,
    span: Span,
) -> Result<CanonicalConst, ConstEvalError> {
    if suffix.is_none() || suffix == Some("d") || suffix == Some("dec") {
        let (coefficient, scale) = parse_decimal(text)
            .ok_or_else(|| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
        let value = ConstValue::decimal(coefficient, scale as u32)
            .map_err(|_| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
        return Ok(make("Decimal", value));
    }
    let (width, bits) = match suffix {
        Some("f" | "f64") => {
            let value = text
                .parse::<f64>()
                .map_err(|_| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
            if !value.is_finite() {
                return Err(ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral));
            }
            (FloatWidth::F64, u128::from(value.to_bits()))
        }
        Some("f32") => {
            let wide = text
                .parse::<f64>()
                .map_err(|_| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
            if !wide.is_finite() || wide.abs() > f64::from(f32::MAX) {
                return Err(ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral));
            }
            (FloatWidth::F32, u128::from((wide as f32).to_bits()))
        }
        Some("f16") => {
            let wide = text
                .parse::<f64>()
                .map_err(|_| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
            let bits = zirk_runtime_core::scalar::float16_bits(wide);
            if !wide.is_finite() || bits & 0x7fff == 0x7c00 {
                return Err(ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral));
            }
            (FloatWidth::F16, u128::from(bits))
        }
        Some("f128") => {
            let value = zirk_runtime_core::binary_float::Float128::from_decimal(text)
                .map_err(|_| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
            (FloatWidth::F128, value.to_bits())
        }
        _ => {
            return Err(ConstEvalError::at(
                span,
                ConstEvalErrorKind::UnsupportedExpression,
            ));
        }
    };
    let value = ConstValue::float(width, bits)
        .map_err(|_| ConstEvalError::at(span, ConstEvalErrorKind::InvalidLiteral))?;
    Ok(make(float_type_identity(width), value))
}

fn parse_decimal(text: &str) -> Option<(i128, u8)> {
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], text[index + 1..].parse::<i32>().ok()?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let mut coefficient = digits.parse::<i128>().ok()?;
    let mut scale = i32::try_from(fraction.len()).ok()?.checked_sub(exponent)?;
    while scale < 0 {
        coefficient = coefficient.checked_mul(10)?;
        scale += 1;
    }
    if scale > 38 {
        return None;
    }
    if coefficient == 0 {
        return Some((0, 0));
    }
    while scale > 0 && coefficient % 10 == 0 {
        coefficient /= 10;
        scale -= 1;
    }
    Some((coefficient, u8::try_from(scale).ok()?))
}

fn display_value(value: &CanonicalConst) -> Option<String> {
    match &value.value {
        ConstValue::Boolean(value) => Some(value.to_string()),
        ConstValue::Integer { value, .. } => Some(value.to_string()),
        ConstValue::BigUnsigned { value, .. } => Some(value.to_string()),
        ConstValue::Decimal { coefficient, scale } => Some(format_decimal(*coefficient, *scale)),
        ConstValue::String(value) | ConstValue::Char(value) => Some(value.clone()),
        ConstValue::Duration(value) => Some(format!("{value}ns")),
        ConstValue::Instant(value) => Some(format!("Instant({value}ns)")),
        ConstValue::TimeZone { identifier, .. } => Some(identifier.clone()),
        ConstValue::ZonedDateTime {
            instant_nanos,
            identifier,
            rule_data,
        } => Some(format!(
            "ZonedDateTime({instant_nanos}ns, {identifier}, tzdb {rule_data})"
        )),
        ConstValue::Period {
            years,
            months,
            weeks,
            days,
        } => Some(format!("P{years}Y{months}M{weeks}W{days}D")),
        _ => None,
    }
}

fn format_decimal(coefficient: i128, scale: u8) -> String {
    if scale == 0 {
        return coefficient.to_string();
    }
    let negative = coefficient < 0;
    let digits = coefficient.unsigned_abs().to_string();
    let scale = usize::from(scale);
    let padded = if digits.len() <= scale {
        format!("{}{}", "0".repeat(scale + 1 - digits.len()), digits)
    } else {
        digits
    };
    let split = padded.len() - scale;
    format!(
        "{}{}.{}",
        if negative { "-" } else { "" },
        &padded[..split],
        &padded[split..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_budget::TypeComputationBudget;
    use zirk_ast::{
        Arg, BinaryExpr, CallExpr, CharLit, DurationLit, FloatLit, Ident, IntLit, Stmt,
    };
    use zirk_diagnostics::{DiagnosticSink, SourceFile, SourceMap, Span};
    use zirk_lexer::tokenize;
    use zirk_parser::parse;

    const S: Span = Span::new(0, 1);

    fn int(value: i128) -> Expr {
        Expr::Int(IntLit { value, span: S })
    }

    fn string(value: &str) -> Expr {
        Expr::Str(zirk_ast::StrLit {
            value: value.into(),
            span: S,
        })
    }

    fn float(text: &str, width: Option<&str>) -> Expr {
        Expr::Float(FloatLit {
            text: text.into(),
            width: width.map(str::to_string),
            span: S,
        })
    }

    fn duration(nanos: i64) -> Expr {
        Expr::Duration(DurationLit { nanos, span: S })
    }

    fn call(name: &str, arguments: Vec<Expr>) -> Expr {
        Expr::Call(CallExpr {
            callee: Box::new(Expr::Path(Ident::new(name, S))),
            type_args: Vec::new(),
            args: arguments.into_iter().map(Arg::positional).collect(),
            span: S,
        })
    }

    /// Adapts the historical `const(...)` test fixture into an ordinary local
    /// initializer before invoking the production parser. The wrapper is test
    /// data only; singleton type syntax no longer accepts it.
    fn parse_constant(source: &str) -> Expr {
        let wrapper = source.find("const(").expect("constant fixture wrapper");
        let expression_start = wrapper + "const(".len();
        let bytes = source.as_bytes();
        let mut depth = 1usize;
        let mut quote = None;
        let mut escaped = false;
        let mut expression_end = None;
        for (offset, byte) in bytes.iter().enumerate().skip(expression_start) {
            if let Some(delimiter) = quote {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == delimiter {
                    quote = None;
                }
                continue;
            }
            if matches!(byte, b'"' | b'\'') {
                quote = Some(*byte);
            } else if *byte == b'(' {
                depth += 1;
            } else if *byte == b')' {
                depth -= 1;
                if depth == 0 {
                    expression_end = Some(offset);
                    break;
                }
            }
        }
        let expression = &source[expression_start..expression_end.expect("closed fixture")];
        let declaration_start = source[..wrapper].rfind("type ").unwrap_or(0);
        let prefix = &source[..declaration_start];
        let transformed =
            format!("{prefix}\nfn __constant_fixture(): Void {{ mut __value = {expression}; }}");
        let mut sources = SourceMap::new();
        sources.add(SourceFile::new("const.zrk", transformed));
        let mut sink = DiagnosticSink::new();
        let file = sources.entry();
        let tokens = tokenize(file, &mut sink);
        let program = parse(file, &tokens, &mut sink);
        assert!(
            !sink.has_errors(),
            "{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );
        let fixture = program
            .functions
            .iter()
            .find(|function| function.name.name == "__constant_fixture")
            .expect("fixture function");
        let Some(Stmt::Let(binding)) = fixture.body.statements.first() else {
            panic!("fixture body should begin with an initialized local");
        };
        binding.init.as_ref().expect("fixture initializer").clone()
    }

    #[test]
    fn checked_integer_arithmetic_has_exact_width_and_value() {
        let expression = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(int(40)),
            right: Box::new(int(2)),
            op_span: S,
            span: S,
        });
        let result = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(result.type_identity, "Int32");
        assert_eq!(
            result.value,
            ConstValue::Integer {
                width: IntWidth::I32,
                value: 42
            }
        );
    }

    #[test]
    fn tuple_constants_preserve_element_types_and_nested_values() {
        let expression = parse_constant("type Pair = const((3, (true, \"ready\")));\n");
        let value = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(value.type_identity, "Tuple<Int32, Tuple<Boolean, String>>");
        let ConstValue::Tuple {
            tuple_identity,
            elements,
        } = value.value
        else {
            panic!("expected canonical tuple value");
        };
        assert_eq!(tuple_identity, "Tuple<Int32, Tuple<Boolean, String>>");
        assert_eq!(elements.len(), 2);
        assert!(matches!(
            elements[0].value,
            ConstValue::Integer { value: 3, .. }
        ));
        let ConstValue::Tuple { elements, .. } = &elements[1].value else {
            panic!("expected nested tuple value");
        };
        assert!(matches!(elements[0].value, ConstValue::Boolean(true)));
        assert!(matches!(elements[1].value, ConstValue::String(ref s) if s == "ready"));
    }

    #[test]
    fn immutable_object_constants_are_recursively_typed_and_order_canonical() {
        let first = parse_constant("type A = const({ x: 3, nested: (true, \"ready\") });");
        let reordered = parse_constant("type A = const({ nested: (true, \"ready\"), x: 3 });");
        let first = evaluate(&first, &mut TypeComputationBudget::default()).unwrap();
        let reordered = evaluate(&reordered, &mut TypeComputationBudget::default()).unwrap();

        assert_eq!(first, reordered);
        assert_eq!(
            first.type_identity,
            "Object<{nested: Tuple<Boolean, String>, x: Int32}>"
        );
        let ConstValue::Aggregate { fields, .. } = first.value else {
            panic!("expected canonical immutable aggregate");
        };
        assert_eq!(
            fields
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["nested", "x"]
        );
        assert!(matches!(fields[0].1.value, ConstValue::Tuple { .. }));
    }

    #[test]
    fn immutable_object_constants_reject_spreads_and_computed_keys() {
        for source in [
            "type A = const({ ...source, x: 1 });",
            "type A = const({ \"{key}\": 1 });",
        ] {
            let expression = parse_constant(source);
            let error = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap_err();
            assert!(matches!(
                error.kind,
                ConstEvalErrorKind::InvalidLiteral | ConstEvalErrorKind::UnsupportedExpression
            ));
        }
    }

    #[test]
    fn immutable_object_constants_reject_mutable_aliases_and_effects_recursively() {
        for source in [
            "type A = const({ nested: { value: mutable_global } });",
            "type A = const({ nested: { value: IO.readLine() } });",
        ] {
            let expression = parse_constant(source);
            let error = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap_err();
            assert!(
                matches!(error.kind, ConstEvalErrorKind::DisallowedEffect(_)),
                "nested paths and effects must fail closed: {source}: {:?}",
                error.kind
            );
        }
    }

    #[test]
    fn enum_constants_preserve_variant_identity_and_typed_payloads() {
        let expression = parse_constant(
            "enum Color { Red, Blue }\n\
             enum Shape { Circle(radius: Int32, color: Color), Point }\n\
             type Example = const(Shape.Circle(3, Color.Red));\n\
             fn main(): Void { }",
        );
        let variants = HashMap::from([
            (
                "Color.Red".into(),
                ConstEnumVariant {
                    enum_identity: "Color".into(),
                    variant_identity: "Color::Red".into(),
                    payload_types: Vec::new(),
                },
            ),
            (
                "Shape.Circle".into(),
                ConstEnumVariant {
                    enum_identity: "Shape".into(),
                    variant_identity: "Shape::Circle".into(),
                    payload_types: vec!["Int32".into(), "Color".into()],
                },
            ),
        ]);
        let value = evaluate_with_enum_variants(
            &expression,
            &mut TypeComputationBudget::default(),
            &HashMap::new(),
            &variants,
        )
        .unwrap();
        assert_eq!(value.type_identity, "Shape");
        let ConstValue::Enum {
            enum_identity,
            variant_identity,
            fields,
        } = value.value
        else {
            panic!("expected canonical enum variant");
        };
        assert_eq!(enum_identity, "Shape");
        assert_eq!(variant_identity, "Shape::Circle");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].type_identity, "Int32");
        let ConstValue::Enum {
            enum_identity,
            variant_identity,
            fields,
        } = &fields[1].value
        else {
            panic!("expected nested enum payload");
        };
        assert_eq!(enum_identity, "Color");
        assert_eq!(variant_identity, "Color::Red");
        assert!(fields.is_empty());
    }

    #[test]
    fn enum_constant_rejects_payload_arity_or_type_mismatches() {
        let variants = HashMap::from([(
            "Shape.Circle".into(),
            ConstEnumVariant {
                enum_identity: "Shape".into(),
                variant_identity: "Shape::Circle".into(),
                payload_types: vec!["Int32".into()],
            },
        )]);
        for source in [
            "enum Shape { Circle(radius: Int32) } type A = const(Shape.Circle);",
            "enum Shape { Circle(radius: Int32) } type A = const(Shape.Circle(true));",
        ] {
            let expression = parse_constant(source);
            let error = evaluate_with_enum_variants(
                &expression,
                &mut TypeComputationBudget::default(),
                &HashMap::new(),
                &variants,
            )
            .unwrap_err();
            assert_eq!(error.kind, ConstEvalErrorKind::TypeMismatch);
        }
    }

    #[test]
    fn equivalent_decimal_spellings_share_the_same_canonical_encoding() {
        let one_tenth =
            evaluate(&float("1.0", None), &mut TypeComputationBudget::default()).unwrap();
        let padded_tenth =
            evaluate(&float("1.00", None), &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(one_tenth.encode(), padded_tenth.encode());

        let parsed_short = parse_constant("type A = const(1.0); fn main(): Void { }");
        let parsed_padded = parse_constant("type A = const(1.00); fn main(): Void { }");
        assert_eq!(
            evaluate(&parsed_short, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&parsed_padded, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );
    }

    #[test]
    fn scalar_arithmetic_matches_decimal_promotion_and_division_rules() {
        let integer_quotient = parse_constant("type A = const(3 / 4); fn main(): Void { }");
        let quotient = evaluate(&integer_quotient, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(quotient.type_identity, "Decimal");
        assert_eq!(
            quotient.value,
            ConstValue::Decimal {
                coefficient: 75,
                scale: 2
            }
        );

        let repeating = parse_constant("type A = const(1.0 / 3.0); fn main(): Void { }");
        let repeated_value = evaluate(&repeating, &mut TypeComputationBudget::default()).unwrap();
        let expected = zirk_runtime_core::decimal::div(
            zirk_runtime_core::decimal::Decimal::new(1, 0),
            zirk_runtime_core::decimal::Decimal::new(3, 0),
        )
        .unwrap();
        assert_eq!(
            repeated_value.value,
            ConstValue::Decimal {
                coefficient: expected.coef,
                scale: expected.scale,
            }
        );

        let mixed = parse_constant("type A = const(1 + 0.5); fn main(): Void { }");
        assert_eq!(
            evaluate(&mixed, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Decimal {
                coefficient: 15,
                scale: 1
            }
        );

        let mixed_decimal_division = parse_constant("type A = const(3 / 4.0); fn main(): Void { }");
        assert_eq!(
            evaluate(
                &mixed_decimal_division,
                &mut TypeComputationBudget::default()
            )
            .unwrap()
            .value,
            ConstValue::Decimal {
                coefficient: 75,
                scale: 2
            }
        );

        let float_widening = parse_constant("type A = const(1.5f32 + 2.0f64); fn main(): Void { }");
        assert_eq!(
            evaluate(&float_widening, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Float {
                width: FloatWidth::F64,
                bits: 3.5f64.to_bits() as u128,
            }
        );

        let float_literal_adopts_sibling =
            parse_constant("type A = const(1 + 2.5f32); fn main(): Void { }");
        assert_eq!(
            evaluate(
                &float_literal_adopts_sibling,
                &mut TypeComputationBudget::default()
            )
            .unwrap()
            .value,
            ConstValue::Float {
                width: FloatWidth::F32,
                bits: 3.5f32.to_bits() as u128,
            }
        );

        let divide_by_zero = parse_constant("type A = const(1 / 0); fn main(): Void { }");
        assert_eq!(
            evaluate(&divide_by_zero, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidLiteral
        );

        let decimal_overflow = parse_constant(
            "type A = const(9999999999999999999999999999999999999.9 * 100.0); fn main(): Void { }",
        );
        assert_eq!(
            evaluate(&decimal_overflow, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::Overflow
        );

        let signed_widening =
            parse_constant("type A = const(Int8(1) + Int32(2)); fn main(): Void { }");
        let widened = evaluate(&signed_widening, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(widened.type_identity, "Int32");
        assert_eq!(
            widened.value,
            ConstValue::Integer {
                width: IntWidth::I32,
                value: 3,
            }
        );

        let unsigned_widening =
            parse_constant("type A = const(UInt8(255) + UInt16(1)); fn main(): Void { }");
        let widened = evaluate(&unsigned_widening, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(widened.type_identity, "UInt16");
        assert_eq!(
            widened.value,
            ConstValue::Integer {
                width: IntWidth::U16,
                value: 256,
            }
        );

        let narrow_overflow =
            parse_constant("type A = const(Int8(127) + Int8(1)); fn main(): Void { }");
        assert_eq!(
            evaluate(&narrow_overflow, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::Overflow
        );

        let incompatible =
            parse_constant("type A = const(Int32(1) + UInt32(1)); fn main(): Void { }");
        assert_eq!(
            evaluate(&incompatible, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::TypeMismatch
        );
    }

    #[test]
    fn float128_literals_and_arithmetic_keep_binary128_precision() {
        let expression = parse_constant("type A = const(1.0f128 + 2.0f128); fn main(): Void { }");
        let value = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(value.type_identity, "Float128");
        let expected = zirk_runtime_core::binary_float::Float128::from_decimal("3.0").unwrap();
        assert_eq!(
            value.value,
            ConstValue::Float {
                width: FloatWidth::F128,
                bits: expected.to_bits()
            }
        );
        let widened = parse_constant("type A = const(0.1f64 + 0.2f128); fn main(): Void { }");
        assert_eq!(
            evaluate(&widened, &mut TypeComputationBudget::default())
                .unwrap()
                .type_identity,
            "Float128"
        );
    }

    #[test]
    fn effectful_and_unverified_calls_fail_at_the_call_site() {
        for (call, expected) in [
            ("IO.readLine()", "I/O"),
            ("Env.get(\"HOME\")", "environment access"),
            ("File.read(\"data\")", "filesystem access"),
            ("Http.get(\"https://example.com\")", "network access"),
            ("Clock.now()", "clock access"),
            ("Random.next()", "randomness"),
            ("Task.sleep(1)", "task or thread creation"),
            ("unknown_native()", "not declared and verified"),
        ] {
            let expression =
                parse_constant(&format!("type A = const({call}); fn main(): Void {{ }}"));
            let error = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap_err();
            let ConstEvalErrorKind::DisallowedEffect(effect) = error.kind else {
                panic!(
                    "expected effect diagnostic for {call}, got {:?}",
                    error.kind
                );
            };
            assert!(effect.contains(expected), "{call}: {effect}");
            assert_eq!(
                error.span,
                expression.span(),
                "{call} must report its call span"
            );
        }

        let expression = parse_constant("type A = const(config); fn main(): Void { }");
        let error = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap_err();
        assert!(matches!(
            error.kind,
            ConstEvalErrorKind::DisallowedEffect(_)
        ));
    }

    #[test]
    fn const_function_body_effects_are_checked_when_evaluated() {
        let functions = HashMap::from([(
            "impure".into(),
            ConstFunction {
                parameter_names: Vec::new(),
                parameter_types: Vec::new(),
                return_type: "Int32".into(),
                body: call("unknown_native", Vec::new()),
            },
        )]);
        let error = evaluate_with_functions(
            &call("impure", Vec::new()),
            &mut TypeComputationBudget::default(),
            &functions,
        )
        .unwrap_err();
        assert!(matches!(
            error.kind,
            ConstEvalErrorKind::DisallowedEffect(_)
        ));
        assert_eq!(error.span, S);
    }

    #[test]
    fn recursive_const_function_calls_exhaust_the_bounded_call_depth() {
        let functions = HashMap::from([(
            "again".into(),
            ConstFunction {
                parameter_names: Vec::new(),
                parameter_types: Vec::new(),
                return_type: "Int32".into(),
                body: call("again", Vec::new()),
            },
        )]);
        let error = evaluate_with_functions(
            &call("again", Vec::new()),
            &mut TypeComputationBudget::default(),
            &functions,
        )
        .unwrap_err();
        assert!(matches!(
            error.kind,
            ConstEvalErrorKind::BudgetExceeded(BudgetExceeded {
                kind: BudgetKind::ConstantCallDepth,
                ..
            })
        ));
    }

    #[test]
    fn float16_literals_and_arithmetic_round_at_binary16_precision() {
        let expression = parse_constant("type A = const(0.1f16 + 0.2f16); fn main(): Void { }");
        let value = evaluate(&expression, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(value.type_identity, "Float16");
        assert_eq!(
            value.value,
            ConstValue::Float {
                width: FloatWidth::F16,
                bits: u128::from(zirk_runtime_core::scalar::float16_bits(
                    zirk_runtime_core::scalar::round_to_float16(0.1)
                        + zirk_runtime_core::scalar::round_to_float16(0.2),
                )),
            }
        );
        let widened = parse_constant("type A = const(0.1f16 + 0.2f32); fn main(): Void { }");
        assert_eq!(
            evaluate(&widened, &mut TypeComputationBudget::default())
                .unwrap()
                .type_identity,
            "Float32"
        );
    }

    #[test]
    fn unsigned_128_arithmetic_preserves_high_bits_and_checks_overflow() {
        let max = ConstValue::unsigned_integer(IntWidth::U128, u128::MAX).unwrap();
        let one = ConstValue::unsigned_integer(IntWidth::U128, 1).unwrap();
        let max_minus_one = evaluate_unsigned_integer_binary(BinaryOp::Sub, &max, &one)
            .expect("unsigned integers are handled")
            .unwrap();
        assert_eq!(
            max_minus_one.value,
            ConstValue::BigUnsigned {
                width: IntWidth::U128,
                value: u128::MAX - 1,
            }
        );
        let restored = evaluate_unsigned_integer_binary(BinaryOp::Add, &max_minus_one.value, &one)
            .expect("mixed compact and wide UInt128 values are handled")
            .unwrap();
        assert_eq!(restored.encode(), make("UInt128", max.clone()).encode());
        assert_eq!(
            evaluate_unsigned_integer_binary(BinaryOp::Add, &max, &one)
                .expect("unsigned integers are handled")
                .unwrap_err(),
            ConstEvalErrorKind::Overflow
        );
        assert_eq!(
            evaluate_unsigned_integer_binary(BinaryOp::Lt, &max_minus_one.value, &max)
                .expect("unsigned comparison is handled")
                .unwrap()
                .value,
            ConstValue::Boolean(true)
        );
        assert_eq!(
            evaluate_decimal_binary(BinaryOp::Div, max.clone(), one.clone()).unwrap_err(),
            ConstEvalErrorKind::Overflow
        );
        let shift = ConstValue::unsigned_integer(IntWidth::U128, 127).unwrap();
        assert_eq!(
            evaluate_unsigned_integer_binary(BinaryOp::Shl, &one, &shift)
                .expect("UInt128 shift is handled")
                .unwrap()
                .value,
            ConstValue::BigUnsigned {
                width: IntWidth::U128,
                value: 1u128 << 127,
            }
        );

        let source = parse_constant("type A = const(UInt128(1) << 127); fn main(): Void { }");
        assert_eq!(
            evaluate(&source, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::BigUnsigned {
                width: IntWidth::U128,
                value: 1u128 << 127,
            }
        );

        let max_source = parse_constant(
            "type A = const(((UInt128(1) << 127) - UInt128(1)) * UInt128(2) + UInt128(1)); fn main(): Void { }",
        );
        assert_eq!(
            evaluate(&max_source, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::BigUnsigned {
                width: IntWidth::U128,
                value: u128::MAX,
            }
        );
    }

    #[test]
    fn pure_bitwise_and_string_repetition_are_bounded() {
        let complement = parse_constant("type A = const(~1); fn main(): Void { }");
        assert_eq!(
            evaluate(&complement, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Integer {
                width: IntWidth::I32,
                value: -2,
            }
        );

        let shifted = parse_constant("type A = const(1 << 31); fn main(): Void { }");
        assert_eq!(
            evaluate(&shifted, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Integer {
                width: IntWidth::I32,
                value: i32::MIN as i128,
            }
        );

        let invalid_shift = parse_constant("type A = const(1 << 32); fn main(): Void { }");
        assert_eq!(
            evaluate(&invalid_shift, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidLiteral
        );

        let repeated = parse_constant("type A = const(\"zirk\" * 2); fn main(): Void { }");
        assert_eq!(
            evaluate(&repeated, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::String("zirkzirk".into())
        );

        let oversized = parse_constant("type A = const(\"x\" * 1048577); fn main(): Void { }");
        assert!(matches!(
            evaluate(&oversized, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::BudgetExceeded(BudgetExceeded {
                kind: BudgetKind::CanonicalValueBytes,
                ..
            })
        ));
    }

    #[test]
    fn scalar_boolean_string_and_binary_float_operations_are_canonical() {
        let booleans = Expr::Binary(BinaryExpr {
            op: BinaryOp::And,
            left: Box::new(Expr::Bool(zirk_ast::BoolLit {
                value: true,
                span: S,
            })),
            right: Box::new(Expr::Bool(zirk_ast::BoolLit {
                value: false,
                span: S,
            })),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&booleans, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Boolean(false)
        );

        let sum = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(float("1.5", Some("f64"))),
            right: Box::new(float("2.25", Some("f64"))),
            op_span: S,
            span: S,
        });
        let result = evaluate(&sum, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(result.type_identity, "Float64");
        assert_eq!(
            result.value,
            ConstValue::Float {
                width: FloatWidth::F64,
                bits: 3.75f64.to_bits() as u128
            }
        );
    }

    #[test]
    fn float_nan_results_are_rejected_during_constant_evaluation() {
        let quotient = Expr::Binary(BinaryExpr {
            op: BinaryOp::Div,
            left: Box::new(float("0.0", Some("f64"))),
            right: Box::new(float("0.0", Some("f64"))),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&quotient, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidFloatResult
        );
    }

    #[test]
    fn date_time_and_datetime_constants_share_runtime_temporal_rules() {
        let date = call("Date", vec![int(2024), int(2), int(29)]);
        let time = call("Time", vec![int(9), int(30), int(0)]);
        let datetime = call("DateTime", vec![date.clone(), time.clone()]);
        let components = call(
            "DateTime",
            vec![int(2024), int(2), int(29), int(9), int(30), int(0)],
        );
        let mut budget = TypeComputationBudget::default();
        let date = evaluate(&date, &mut budget).unwrap();
        assert_eq!(
            date.value,
            ConstValue::Date(zirk_runtime_core::temporal::days_from_civil(2024, 2, 29))
        );
        let datetime = evaluate(&datetime, &mut TypeComputationBudget::default()).unwrap();
        let components = evaluate(&components, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(datetime.encode(), components.encode());
    }

    #[test]
    fn zone_aware_temporal_constants_pin_and_validate_rule_data() {
        let zone = evaluate(
            &call("TimeZone", vec![string("America/Santiago")]),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert_eq!(zone.type_identity, "TimeZone");
        let ConstValue::TimeZone {
            identifier,
            rule_data,
        } = &zone.value
        else {
            panic!("expected a canonical TimeZone value")
        };
        assert_eq!(identifier, "America/Santiago");
        assert_eq!(rule_data, chrono_tz::IANA_TZDB_VERSION);
        let alias = evaluate(
            &call("TimeZone", vec![string("US/Pacific")]),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert_eq!(
            alias.value,
            ConstValue::TimeZone {
                identifier: "America/Los_Angeles".into(),
                rule_data: chrono_tz::IANA_TZDB_VERSION.into(),
            }
        );

        let reported_version = evaluate(
            &call("TimeZone.database_version", vec![]),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert_eq!(
            reported_version.value,
            ConstValue::String(chrono_tz::IANA_TZDB_VERSION.to_string())
        );

        let instant = evaluate(
            &call("Instant.from_unix_seconds", vec![int(-1)]),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert_eq!(instant.value, ConstValue::Instant(-1_000_000_000));
        let zoned = evaluate(
            &call(
                "ZonedDateTime.from_instant",
                vec![
                    call("Instant.from_unix_seconds", vec![int(-1)]),
                    call("TimeZone", vec![string("America/Santiago")]),
                ],
            ),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        let ConstValue::ZonedDateTime {
            instant_nanos,
            identifier: zone_id,
            rule_data: zoned_rules,
        } = zoned.value
        else {
            panic!("expected a canonical ZonedDateTime value")
        };
        assert_eq!(instant_nanos, -1_000_000_000);
        assert_eq!(zone_id, "America/Santiago");
        assert_eq!(zoned_rules, chrono_tz::IANA_TZDB_VERSION);

        let method_form = parse_constant(
            "type Local = const(Instant.from_unix_seconds(-1).in_zone(TimeZone.UTC)); fn main(): Void { }",
        );
        assert_eq!(
            evaluate(&method_form, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::ZonedDateTime {
                instant_nanos: -1_000_000_000,
                identifier: parse_iana_zone("UTC").unwrap(),
                rule_data: chrono_tz::IANA_TZDB_VERSION.into(),
            }
        );
        assert_eq!(
            evaluate(
                &parse_constant(
                    "type Tick = const(Instant.parse(\"1970-01-01T00:00:01Z\")); fn main(): Void { }",
                ),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Instant(1_000_000_000)
        );
        assert_eq!(
            evaluate(
                &parse_constant(
                    "type Later = const(Instant.from_unix_seconds(1) + 1s); fn main(): Void { }",
                ),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Instant(2_000_000_000)
        );
        let summer_instant = evaluate(
            &call("Instant.parse", vec![string("2024-01-01T12:00:00Z")]),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert!(zone_instant_info(&zone, &summer_instant).is_some());
        assert_eq!(
            evaluate(
                &parse_constant(
                    "type Offset = const(TimeZone(\"America/Santiago\").offset_at(Instant.parse(\"2024-01-01T12:00:00Z\"))); fn main(): Void { }",
                ),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Duration(-3 * 60 * 60 * 1_000_000_000)
        );
        assert_eq!(
            evaluate(
                &parse_constant(
                    "type Daylight = const(TimeZone(\"America/Santiago\").is_dst_at(Instant.parse(\"2024-01-01T12:00:00Z\"))); fn main(): Void { }",
                ),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Boolean(true)
        );

        let period_from_iso = evaluate(
            &call("Period.parse", vec![string("P1Y2M3D")]),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        let period_from_components = evaluate(
            &parse_constant(
                "type Calendar = const(Period(years: 1, months: 2, days: 3)); fn main(): Void { }",
            ),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert_eq!(period_from_iso, period_from_components);
        assert_eq!(
            evaluate(
                &parse_constant("type Calendar = const(Period(years: 1, months: 2, days: 3)); fn main(): Void { }"),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Period {
                years: 1,
                months: 2,
                weeks: 0,
                days: 3,
            }
        );
        assert_eq!(
            evaluate(
                &parse_constant(
                    "type Calendar = const(Period.months(1) + Period.days(2)); fn main(): Void { }",
                ),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Period {
                years: 0,
                months: 1,
                weeks: 0,
                days: 2,
            }
        );
        assert_eq!(
            evaluate(
                &parse_constant("type Calendar = const(-Period.months(1)); fn main(): Void { }",),
                &mut TypeComputationBudget::default(),
            )
            .unwrap()
            .value,
            ConstValue::Period {
                years: 0,
                months: -1,
                weeks: 0,
                days: 0,
            }
        );

        let parsed = evaluate(
            &call(
                "ZonedDateTime.parse",
                vec![string("2024-07-01T12:00:00-04:00[America/Santiago]")],
            ),
            &mut TypeComputationBudget::default(),
        )
        .unwrap();
        assert_eq!(
            parsed.value,
            ConstValue::ZonedDateTime {
                instant_nanos: 1_719_849_600_000_000_000,
                identifier: "America/Santiago".into(),
                rule_data: chrono_tz::IANA_TZDB_VERSION.into(),
            }
        );

        assert!(!rule_data_matches_current("stale-zone-data"));
        assert!(rule_data_matches_current(chrono_tz::IANA_TZDB_VERSION));
        let stale_args = HashMap::from([
            ("instant".into(), make("Instant", ConstValue::Instant(0))),
            (
                "zone".into(),
                make(
                    "TimeZone",
                    ConstValue::TimeZone {
                        identifier: "America/Santiago".into(),
                        rule_data: "stale-zone-data".into(),
                    },
                ),
            ),
        ]);
        let stale_call = Expr::Call(CallExpr {
            callee: Box::new(Expr::Path(Ident::new("ZonedDateTime.from_instant", S))),
            type_args: Vec::new(),
            args: vec![
                Arg::positional(Expr::Path(Ident::new("instant", S))),
                Arg::positional(Expr::Path(Ident::new("zone", S))),
            ],
            span: S,
        });
        assert_eq!(
            evaluate_inner(
                &stale_call,
                &mut TypeComputationBudget::default(),
                &HashMap::new(),
                &HashMap::new(),
                &stale_args,
            )
            .unwrap_err()
            .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
        assert_eq!(
            evaluate(
                &call("TimeZone", vec![string("Mars/Olympus")]),
                &mut TypeComputationBudget::default(),
            )
            .unwrap_err()
            .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
        assert_eq!(
            evaluate(
                &call(
                    "ZonedDateTime.parse",
                    vec![string("2024-07-01T12:00:00-03:00[America/Santiago]")]
                ),
                &mut TypeComputationBudget::default(),
            )
            .unwrap_err()
            .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
        assert!(matches!(
            evaluate(
                &parse_constant("type Zone = const(TimeZone.system()); fn main(): Void { }"),
                &mut TypeComputationBudget::default(),
            )
            .unwrap_err()
            .kind,
            ConstEvalErrorKind::DisallowedEffect(_)
        ));
    }

    #[test]
    fn invalid_temporal_constructor_values_are_rejected() {
        let invalid_date = call("Date", vec![int(2024), int(2), int(30)]);
        assert_eq!(
            evaluate(&invalid_date, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
        let invalid_time = call("Time", vec![int(24), int(0)]);
        assert_eq!(
            evaluate(&invalid_time, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
    }

    #[test]
    fn civil_temporal_arithmetic_uses_runtime_day_and_wrap_rules() {
        let date = call("Date", vec![int(2024), int(2), int(29)]);
        let next_date = call("Date", vec![int(2024), int(3), int(1)]);
        let plus_day = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(date.clone()),
            right: Box::new(int(1)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&plus_day, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&next_date, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );

        let late = call("Time", vec![int(23), int(30)]);
        let wrapped = call("Time", vec![int(1), int(30)]);
        let plus_two_hours = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(late),
            right: Box::new(duration(7_200_000_000_000)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&plus_two_hours, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&wrapped, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );

        let dt = call("DateTime", vec![date, call("Time", vec![int(23), int(0)])]);
        let next_dt = call(
            "DateTime",
            vec![next_date, call("Time", vec![int(1), int(0)])],
        );
        let shift = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(dt),
            right: Box::new(duration(2 * zirk_runtime_core::temporal::NANOS_PER_HOUR)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&shift, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&next_dt, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );
    }

    #[test]
    fn temporal_text_parsing_matches_runtime_and_rejects_invalid_text() {
        let parsed_date =
            parse_constant("type A = const(Date.parse(\"2024-02-29\")); fn main(): Void { }");
        let constructed_date = call("Date", vec![int(2024), int(2), int(29)]);
        assert_eq!(
            evaluate(&parsed_date, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&constructed_date, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );

        let parsed_time =
            parse_constant("type A = const(Time.parse(\"09:30:00\")); fn main(): Void { }");
        let constructed_time = call("Time", vec![int(9), int(30), int(0)]);
        assert_eq!(
            evaluate(&parsed_time, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&constructed_time, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );

        let parsed_datetime = parse_constant(
            "type A = const(DateTime.parse(\"2024-02-29T09:30:00\")); fn main(): Void { }",
        );
        let constructed_datetime = call(
            "DateTime",
            vec![
                call("Date", vec![int(2024), int(2), int(29)]),
                call("Time", vec![int(9), int(30), int(0)]),
            ],
        );
        assert_eq!(
            evaluate(&parsed_datetime, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(&constructed_datetime, &mut TypeComputationBudget::default())
                .unwrap()
                .encode()
        );

        let invalid =
            parse_constant("type A = const(Date.parse(\"2024-02-30\")); fn main(): Void { }");
        assert_eq!(
            evaluate(&invalid, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
    }

    #[test]
    fn char_constants_obey_the_language_grapheme_contract() {
        let valid = Expr::Char(CharLit {
            value: "e\u{301}".into(),
            span: S,
        });
        assert_eq!(
            evaluate(&valid, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Char("e\u{301}".into())
        );
        let invalid = Expr::Char(CharLit {
            value: "ab".into(),
            span: S,
        });
        assert_eq!(
            evaluate(&invalid, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::InvalidLiteral
        );
    }

    #[test]
    fn duration_spellings_and_arithmetic_use_exact_nanoseconds() {
        let equivalent_sum = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(duration(250_000_000)),
            right: Box::new(duration(750_000_000)),
            op_span: S,
            span: S,
        });
        let result = evaluate(&equivalent_sum, &mut TypeComputationBudget::default()).unwrap();
        assert_eq!(result.value, ConstValue::Duration(1_000_000_000));
        assert_eq!(
            result.encode(),
            make("Duration", ConstValue::Duration(1_000_000_000)).encode()
        );

        let parsed_millis = parse_constant("type A = const(250ms); fn main(): Void { }");
        let parsed_fractional_seconds =
            parse_constant("type A = const(0.25s); fn main(): Void { }");
        assert_eq!(
            evaluate(&parsed_millis, &mut TypeComputationBudget::default())
                .unwrap()
                .encode(),
            evaluate(
                &parsed_fractional_seconds,
                &mut TypeComputationBudget::default()
            )
            .unwrap()
            .encode()
        );

        let scaled = Expr::Binary(BinaryExpr {
            op: BinaryOp::Mul,
            left: Box::new(duration(500_000_000)),
            right: Box::new(int(2)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&scaled, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Duration(1_000_000_000)
        );

        let integer_division = Expr::Binary(BinaryExpr {
            op: BinaryOp::Div,
            left: Box::new(duration(1_000_000_000)),
            right: Box::new(int(3)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&integer_division, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Duration(333_333_333)
        );

        let float_ratio = Expr::Binary(BinaryExpr {
            op: BinaryOp::Div,
            left: Box::new(duration(1_000_000_000)),
            right: Box::new(duration(500_000_000)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&float_ratio, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Float {
                width: FloatWidth::F64,
                bits: 2.0f64.to_bits() as u128
            }
        );

        let float_scale = Expr::Binary(BinaryExpr {
            op: BinaryOp::Div,
            left: Box::new(duration(1_000_000_000)),
            right: Box::new(float("2.0", Some("f64"))),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&float_scale, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Duration(500_000_000)
        );

        let decimal_scale = parse_constant("type A = const(2s * 1.5); fn main(): Void { }");
        assert_eq!(
            evaluate(&decimal_scale, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Duration(3_000_000_000)
        );

        let negative = Expr::Unary(zirk_ast::UnaryExpr {
            op: UnaryOp::Neg,
            operand: Box::new(duration(1_000_000_000)),
            span: S,
        });
        assert_eq!(
            evaluate(&negative, &mut TypeComputationBudget::default())
                .unwrap()
                .value,
            ConstValue::Duration(-1_000_000_000)
        );
    }

    #[test]
    fn arithmetic_overflow_and_exhausted_fuel_fail_deterministically() {
        let overflowing = Expr::Binary(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(int(i32::MAX as i128)),
            right: Box::new(int(1)),
            op_span: S,
            span: S,
        });
        assert_eq!(
            evaluate(&overflowing, &mut TypeComputationBudget::default())
                .unwrap_err()
                .kind,
            ConstEvalErrorKind::Overflow
        );

        let mut budget = TypeComputationBudget::default();
        budget
            .charge(BudgetKind::ConstantFuel, BudgetKind::ConstantFuel.limit())
            .unwrap();
        assert!(matches!(
            evaluate(&int(1), &mut budget).unwrap_err().kind,
            ConstEvalErrorKind::BudgetExceeded(BudgetExceeded {
                kind: BudgetKind::ConstantFuel,
                ..
            })
        ));

        let mut nested = int(1);
        for _ in 0..BudgetKind::ConstantCallDepth.limit() + 1 {
            nested = Expr::Unary(zirk_ast::UnaryExpr {
                op: UnaryOp::BitNot,
                operand: Box::new(nested),
                span: S,
            });
        }
        let error = evaluate(&nested, &mut TypeComputationBudget::default()).unwrap_err();
        assert_eq!(error.span, S);
        assert!(matches!(
            error.kind,
            ConstEvalErrorKind::BudgetExceeded(BudgetExceeded {
                kind: BudgetKind::ConstantCallDepth,
                limit: 32,
                ..
            })
        ));
    }
}
