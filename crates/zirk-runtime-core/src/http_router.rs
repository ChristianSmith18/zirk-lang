//! Deterministic, bounded HTTP route-pattern matching shared by the native
//! server adapter and its conformance tests.

use percent_encoding::percent_decode_str;

use crate::http_values::HttpMethod;

pub const MAX_HTTP_ROUTES: usize = 1_024;
pub const MAX_HTTP_ROUTE_PATTERN_BYTES: usize = 2_048;
pub const MAX_HTTP_ROUTE_PARAMETERS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRouteError {
    InvalidPattern,
    InvalidPath,
    DuplicateRoute,
    AmbiguousRoute,
    RouteLimit,
    InvalidParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Static(String),
    Parameter(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Route {
    method: HttpMethod,
    pattern: String,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRouteMatch {
    /// Stable zero-based registration index for the matched route.
    pub route_index: usize,
    /// Decoded parameter values in pattern order.
    pub parameters: Vec<(String, String)>,
}

/// Validates one route pattern with the same bounds and segment rules used by
/// [`HttpRouteTable::register`].
pub fn validate_route_pattern(pattern: &str) -> Result<(), HttpRouteError> {
    parse_pattern(pattern).map(|_| ())
}

/// Checks whether two patterns for the same method conflict. This is useful
/// to adapters that keep handler values in their own owner-managed table but
/// must share the core router's duplicate and ambiguity rules.
pub fn validate_route_pair(
    method: HttpMethod,
    first: &str,
    second: &str,
) -> Result<(), HttpRouteError> {
    let mut routes = HttpRouteTable::new();
    routes.register(method, first)?;
    routes.register(method, second).map(|_| ())
}

/// Matches one method/pattern pair and returns its decoded parameters. The
/// result carries index zero because the adapter owns the surrounding route
/// index and handler mapping.
pub fn match_route_pattern(
    method: HttpMethod,
    pattern: &str,
    path: &str,
) -> Result<Option<HttpRouteMatch>, HttpRouteError> {
    let mut routes = HttpRouteTable::new();
    routes.register(method, pattern)?;
    routes.resolve(method, path)
}

/// A method-aware route table. Patterns that can match the same path under
/// the same method are rejected at registration, so the selected handler never
/// depends on declaration order.
#[derive(Debug, Clone, Default)]
pub struct HttpRouteTable {
    routes: Vec<Route>,
}

impl HttpRouteTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn register(&mut self, method: HttpMethod, pattern: &str) -> Result<usize, HttpRouteError> {
        let segments = parse_pattern(pattern)?;
        for existing in &self.routes {
            if existing.method == method && patterns_overlap(&existing.segments, &segments) {
                if same_shape(&existing.segments, &segments) {
                    return Err(HttpRouteError::DuplicateRoute);
                }
                if static_segment_count(&existing.segments) == static_segment_count(&segments) {
                    return Err(HttpRouteError::AmbiguousRoute);
                }
            }
        }
        if self.routes.len() >= MAX_HTTP_ROUTES {
            return Err(HttpRouteError::RouteLimit);
        }
        let index = self.routes.len();
        self.routes.push(Route {
            method,
            pattern: pattern.to_owned(),
            segments,
        });
        Ok(index)
    }

    pub fn resolve(
        &self,
        method: HttpMethod,
        path: &str,
    ) -> Result<Option<HttpRouteMatch>, HttpRouteError> {
        let request_segments = parse_request_path(path)?;
        let mut best: Option<(usize, usize)> = None;
        for (route_index, route) in self.routes.iter().enumerate() {
            if route.method != method || route.segments.len() != request_segments.len() {
                continue;
            }
            let mut matches = true;
            for (pattern, request) in route.segments.iter().zip(&request_segments) {
                match pattern {
                    Segment::Static(expected) if expected == request => {}
                    Segment::Static(_) => {
                        matches = false;
                        break;
                    }
                    Segment::Parameter(_) => {}
                }
            }
            if matches {
                let specificity = static_segment_count(&route.segments);
                if best.is_none_or(|(_, best_specificity)| specificity > best_specificity) {
                    best = Some((route_index, specificity));
                }
            }
        }
        let Some((route_index, _)) = best else {
            return Ok(None);
        };
        let route = &self.routes[route_index];
        let parameters = route
            .segments
            .iter()
            .zip(&request_segments)
            .filter_map(|(segment, request)| match segment {
                Segment::Parameter(name) => Some((name.clone(), *request)),
                Segment::Static(_) => None,
            })
            .map(|(name, request)| Ok((name, decode_parameter(request)?)))
            .collect::<Result<Vec<_>, HttpRouteError>>()?;
        Ok(Some(HttpRouteMatch {
            route_index,
            parameters,
        }))
    }

    /// Returns the methods registered for a path, in stable wire-name order.
    /// An empty result means the path is unknown and should produce 404;
    /// otherwise a method miss should produce 405 with this Allow set.
    pub fn allowed_methods(&self, path: &str) -> Result<Vec<HttpMethod>, HttpRouteError> {
        let request_segments = parse_request_path(path)?;
        let mut methods = self
            .routes
            .iter()
            .filter(|route| route_path_matches(&route.segments, &request_segments))
            .map(|route| route.method)
            .collect::<Vec<_>>();
        methods.sort_by_key(|method| method.as_str());
        methods.dedup();
        Ok(methods)
    }

    pub fn pattern(&self, route_index: usize) -> Option<&str> {
        self.routes
            .get(route_index)
            .map(|route| route.pattern.as_str())
    }
}

fn parse_pattern(pattern: &str) -> Result<Vec<Segment>, HttpRouteError> {
    if pattern.len() > MAX_HTTP_ROUTE_PATTERN_BYTES
        || !pattern.starts_with('/')
        || pattern.contains(['?', '#', '\\'])
        || pattern.chars().any(char::is_control)
        || !valid_percent_escapes(pattern)
    {
        return Err(HttpRouteError::InvalidPattern);
    }
    if pattern == "/" {
        return Ok(Vec::new());
    }
    let mut parameters = Vec::new();
    let segments = pattern
        .strip_prefix('/')
        .ok_or(HttpRouteError::InvalidPattern)?
        .split('/')
        .map(|segment| {
            if segment.is_empty() || !segment.is_ascii() {
                return Err(HttpRouteError::InvalidPattern);
            }
            if unsafe_encoded_path_segment(segment) {
                return Err(HttpRouteError::InvalidPattern);
            }
            if let Some(name) = segment.strip_prefix(':') {
                if !valid_parameter_name(name)
                    || parameters.iter().any(|previous| previous == name)
                    || parameters.len() >= MAX_HTTP_ROUTE_PARAMETERS
                {
                    return Err(HttpRouteError::InvalidPattern);
                }
                parameters.push(name.to_owned());
                Ok(Segment::Parameter(name.to_owned()))
            } else if segment.starts_with('*') {
                Err(HttpRouteError::InvalidPattern)
            } else {
                Ok(Segment::Static(segment.to_owned()))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(segments)
}

fn parse_request_path(path: &str) -> Result<Vec<&str>, HttpRouteError> {
    if path.len() > MAX_HTTP_ROUTE_PATTERN_BYTES
        || !path.starts_with('/')
        || path.contains(['?', '#', '\\'])
        || path.chars().any(char::is_control)
        || !valid_percent_escapes(path)
    {
        return Err(HttpRouteError::InvalidPath);
    }
    if path == "/" {
        return Ok(Vec::new());
    }
    let segments = path
        .strip_prefix('/')
        .ok_or(HttpRouteError::InvalidPath)?
        .split('/')
        .collect::<Vec<_>>();
    if segments
        .iter()
        .any(|segment| segment.is_empty() || !segment.is_ascii())
    {
        return Err(HttpRouteError::InvalidPath);
    }
    if segments
        .iter()
        .any(|segment| unsafe_encoded_path_segment(segment))
    {
        return Err(HttpRouteError::InvalidPath);
    }
    Ok(segments)
}

fn unsafe_encoded_path_segment(segment: &str) -> bool {
    let Ok(decoded) = percent_decode_str(segment).decode_utf8() else {
        return true;
    };
    decoded.contains(['/', '\\'])
        || decoded == "."
        || decoded == ".."
        || decoded.chars().any(char::is_control)
}

fn patterns_overlap(left: &[Segment], right: &[Segment]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (Segment::Static(a), Segment::Static(b)) => a == b,
                _ => true,
            })
}

fn same_shape(left: &[Segment], right: &[Segment]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            matches!(
                (left, right),
                (Segment::Static(_), Segment::Static(_))
                    | (Segment::Parameter(_), Segment::Parameter(_))
            ) && match (left, right) {
                (Segment::Static(a), Segment::Static(b)) => a == b,
                (Segment::Parameter(_), Segment::Parameter(_)) => true,
                _ => false,
            }
        })
}

fn static_segment_count(segments: &[Segment]) -> usize {
    segments
        .iter()
        .filter(|segment| matches!(segment, Segment::Static(_)))
        .count()
}

fn route_path_matches(pattern: &[Segment], request: &[&str]) -> bool {
    pattern.len() == request.len()
        && pattern
            .iter()
            .zip(request)
            .all(|(pattern, request)| match pattern {
                Segment::Static(expected) => expected == request,
                Segment::Parameter(_) => true,
            })
}

fn valid_parameter_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn valid_percent_escapes(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

fn decode_parameter(value: &str) -> Result<String, HttpRouteError> {
    let decoded = percent_decode_str(value)
        .decode_utf8()
        .map_err(|_| HttpRouteError::InvalidParameter)?;
    if decoded.is_empty()
        || decoded == "."
        || decoded == ".."
        || decoded.contains(['/', '\\'])
        || decoded.chars().any(char::is_control)
    {
        return Err(HttpRouteError::InvalidParameter);
    }
    Ok(decoded.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_aware_routes_match_and_decode_parameters() {
        let mut routes = HttpRouteTable::new();
        assert_eq!(routes.register(HttpMethod::Get, "/users/:user_id"), Ok(0));
        assert_eq!(routes.register(HttpMethod::Query, "/users/:user_id"), Ok(1));

        let matched = routes
            .resolve(HttpMethod::Get, "/users/alice%20smith")
            .unwrap()
            .unwrap();
        assert_eq!(matched.route_index, 0);
        assert_eq!(
            matched.parameters,
            [("user_id".into(), "alice smith".into())]
        );
        assert_eq!(routes.resolve(HttpMethod::Post, "/users/alice"), Ok(None));
        assert_eq!(routes.pattern(1), Some("/users/:user_id"));
    }

    #[test]
    fn single_route_adapter_helpers_share_validation_and_decoding_rules() {
        assert_eq!(validate_route_pattern("/users/:id"), Ok(()));
        assert_eq!(
            validate_route_pattern("/users/%2F"),
            Err(HttpRouteError::InvalidPattern)
        );
        assert_eq!(
            validate_route_pair(HttpMethod::Get, "/users/:id", "/users/:name"),
            Err(HttpRouteError::DuplicateRoute)
        );
        assert_eq!(
            validate_route_pair(HttpMethod::Get, "/users/:id/posts", "/users/alice/:post"),
            Err(HttpRouteError::AmbiguousRoute)
        );
        let matched = match_route_pattern(HttpMethod::Get, "/users/:id", "/users/a%20b")
            .unwrap()
            .unwrap();
        assert_eq!(matched.route_index, 0);
        assert_eq!(matched.parameters, [("id".into(), "a b".into())]);
        assert_eq!(
            match_route_pattern(HttpMethod::Get, "/users/:id", "/users/a%2Fb"),
            Err(HttpRouteError::InvalidPath)
        );
    }

    #[test]
    fn allowed_methods_distinguish_unknown_paths_from_method_misses() {
        let mut routes = HttpRouteTable::new();
        routes.register(HttpMethod::Post, "/items/:id").unwrap();
        routes.register(HttpMethod::Get, "/items/:id").unwrap();
        assert_eq!(
            routes.allowed_methods("/items/42"),
            Ok(vec![HttpMethod::Get, HttpMethod::Post])
        );
        assert_eq!(routes.allowed_methods("/unknown"), Ok(Vec::new()));
    }

    #[test]
    fn overlapping_routes_are_rejected_independent_of_registration_order() {
        let mut first = HttpRouteTable::new();
        first.register(HttpMethod::Get, "/users/:id/posts").unwrap();
        assert_eq!(
            first.register(HttpMethod::Get, "/users/alice/:post_id"),
            Err(HttpRouteError::AmbiguousRoute)
        );

        let mut reverse = HttpRouteTable::new();
        reverse
            .register(HttpMethod::Get, "/users/alice/:post_id")
            .unwrap();
        assert_eq!(
            reverse.register(HttpMethod::Get, "/users/:id/posts"),
            Err(HttpRouteError::AmbiguousRoute)
        );
    }

    #[test]
    fn duplicate_shapes_are_rejected_and_static_routes_win_by_specificity() {
        let mut routes = HttpRouteTable::new();
        routes.register(HttpMethod::Get, "/users/:id").unwrap();
        assert_eq!(
            routes.register(HttpMethod::Get, "/users/:name"),
            Err(HttpRouteError::DuplicateRoute)
        );
        assert_eq!(routes.register(HttpMethod::Get, "/users/new"), Ok(1));
        assert_eq!(
            routes
                .resolve(HttpMethod::Get, "/users/new")
                .unwrap()
                .unwrap()
                .route_index,
            1
        );
        assert_eq!(
            routes
                .resolve(HttpMethod::Get, "/users/bob")
                .unwrap()
                .unwrap()
                .route_index,
            0
        );

        let mut reverse = HttpRouteTable::new();
        reverse.register(HttpMethod::Get, "/users/new").unwrap();
        reverse.register(HttpMethod::Get, "/users/:id").unwrap();
        assert_eq!(
            reverse
                .resolve(HttpMethod::Get, "/users/new")
                .unwrap()
                .unwrap()
                .route_index,
            0
        );
    }

    #[test]
    fn malformed_patterns_paths_and_encoded_separators_fail_closed() {
        for pattern in [
            "users/:id",
            "/users/",
            "/users//:id",
            "/users/:",
            "/x/%GG",
            "/x/%2F",
            "/x?y",
        ] {
            assert_eq!(
                HttpRouteTable::new().register(HttpMethod::Get, pattern),
                Err(HttpRouteError::InvalidPattern),
                "accepted invalid pattern {pattern:?}"
            );
        }
        let mut routes = HttpRouteTable::new();
        routes.register(HttpMethod::Get, "/files/:name").unwrap();
        for path in ["/files/a%2Fb", "/files/%5Ctmp", "/files/%00", "/files/%FF"] {
            assert_eq!(
                routes.resolve(HttpMethod::Get, path),
                Err(HttpRouteError::InvalidPath),
                "accepted unsafe parameter {path:?}"
            );
        }
        assert_eq!(
            routes.resolve(HttpMethod::Get, "/files/a?admin=true"),
            Err(HttpRouteError::InvalidPath)
        );
    }

    #[test]
    fn route_count_is_bounded() {
        let mut routes = HttpRouteTable::new();
        for index in 0..MAX_HTTP_ROUTES {
            routes
                .register(HttpMethod::Get, &format!("/route-{index}"))
                .unwrap();
        }
        assert_eq!(
            routes.register(HttpMethod::Get, "/one-too-many"),
            Err(HttpRouteError::RouteLimit)
        );
    }
}
