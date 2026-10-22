## ADDED Requirements

### Requirement: HTTP success absence is distinct from HTTP failure
Public HTTP APIs SHALL NOT use `Result<T?, HTTPError>` to represent both a successful absence and an operation failure. Successful domain states such as a direct response without an indirect URL or a completed SSE stream SHALL be represented by explicit non-null domain outcomes inside `Result`; HTTP failures SHALL remain in `Error(HTTPError)`.

#### Scenario: A QUERY response has no indirect result URL
- **WHEN** `HTTPResponse.indirect_result_url(base)` inspects a response that is not a 303 response
- **THEN** it returns `Ok(HTTPIndirectResult.Direct)`

#### Scenario: A QUERY response provides an indirect result URL
- **WHEN** `HTTPResponse.indirect_result_url(base)` resolves exactly one valid Location value
- **THEN** it returns `Ok(HTTPIndirectResult.Redirect(url))`

#### Scenario: An indirect result response is malformed
- **WHEN** the Location metadata is missing, ambiguous, or invalid for a 303 response
- **THEN** it returns `Error(HTTPError)` rather than a successful absent URL

#### Scenario: An SSE stream reaches its end
- **WHEN** `HTTPEventStream.next_event()` reaches clean end-of-stream
- **THEN** it returns `Ok(HTTPEventPoll.End)`

#### Scenario: An SSE stream produces an event
- **WHEN** `HTTPEventStream.next_event()` reads a complete event
- **THEN** it returns `Ok(HTTPEventPoll.Event(event))`

#### Scenario: An SSE stream encounters a transport or protocol failure
- **WHEN** `HTTPEventStream.next_event()` cannot continue because of a typed failure
- **THEN** it returns `Error(HTTPError)` distinct from `HTTPEventPoll.End`
