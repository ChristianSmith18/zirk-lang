## ADDED Requirements

### Requirement: Secret redaction is enforced at every value-output boundary

Every implemented formatter, debugger view, heap or execution snapshot, test result, structured diagnostic, and tool-output serializer that can traverse a `SecretString` SHALL render only its redacted marker and SHALL NOT serialize its private payload. Redaction SHALL apply recursively through collections, captured values, thrown errors, and derived layouts before data is persisted, transported, or displayed. A sink that cannot honor the redaction contract SHALL reject or omit the secret-bearing value. No debugger, snapshotter, test runner, or reporter may be introduced without executable sink-specific redaction coverage.

#### Scenario: Inspect a secret inside a nested value
- **WHEN** an implemented inspection or reporting sink renders a collection or captured object containing a `SecretString`
- **THEN** it recursively emits only the redacted marker and never the raw payload

#### Scenario: Persist a secret-bearing diagnostic
- **WHEN** a structured output sink serializes an error containing a secret
- **THEN** the serialized bytes contain only redacted secret data

#### Scenario: A sink cannot preserve redaction
- **WHEN** an output sink cannot identify and redact a secret-bearing value
- **THEN** it omits or rejects that value instead of serializing its private payload
