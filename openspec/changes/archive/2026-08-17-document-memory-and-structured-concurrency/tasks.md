## 1. Canonical semantic sources

- [x] 1.1 Create the canonical memory, references, and transactional unsafe semantics document
- [x] 1.2 Create the canonical structured concurrency semantics document
- [x] 1.3 Link both new sources from the final specification and agent reading order
- [x] 1.4 Align the language specification with memory, unsafe, task, and select syntax
- [x] 1.5 Align the runtime specification with rollback, task lifetime, failure, cancellation, and scheduling
- [x] 1.6 Align the standard library specification with weak/native-view/task/channel/synchronization APIs
- [x] 1.7 Align the compiler specification with escape, effect, transfer/share, and race analyses

## 2. Memory and unsafe handbook

- [x] 2.1 Expand the memory model and automatic-management chapters
- [x] 2.2 Document safe, weak, dependent, and pinned references
- [x] 2.3 Document pointer construction, nullability, arithmetic, casts, and native views
- [x] 2.4 Document unsafe functions and the closed unsafe operation set
- [x] 2.5 Document transactional journals, rollback triggers, and optimization freedom
- [x] 2.6 Document irreversible commit regions and external-effect restrictions
- [x] 2.7 Document deep clone graph behavior and unsupported members
- [x] 2.8 Document controlled traps, unrecoverable corruption, and sanitizer boundaries

## 3. Structured concurrency handbook

- [x] 3.1 Expand task creation, `Task<T>`, await, and structured scopes
- [x] 3.2 Document failure propagation, ignored results, and supervised services
- [x] 3.3 Document cancellation, shields, and timeout cleanup
- [x] 3.4 Document `Task.all`, `Task.first`, `Task.settled`, and settlements
- [x] 3.5 Add a complete `select` chapter with fairness and losing-operation rules
- [x] 3.6 Expand bounded/unbounded channel behavior and closure outcomes
- [x] 3.7 Document transfer/share derivation and concurrent capture rules
- [x] 3.8 Expand ordered/unordered parallel work and reductions
- [x] 3.9 Expand scoped threads and blocking adapters
- [x] 3.10 Expand mutex, library synchronizer, atomic, and data-race rules

## 4. Reference and integration alignment

- [x] 4.1 Update grammar, keywords, operators, precedence, and type/member references
- [x] 4.2 Add valid and invalid end-to-end memory/unsafe examples
- [x] 4.3 Add valid and invalid end-to-end concurrency examples
- [x] 4.4 Update runtime, native, task, thread, and synchronization library reference pages
- [x] 4.5 Update glossary, feature status, roadmap, and implementation phase dependencies
- [x] 4.6 Update handbook summary, section indexes, previous/next navigation, and reading paths
- [x] 4.7 Reconcile error, resource, permission, collection, and callable documentation at interaction points
- [x] 4.8 Update active OpenSpec designs/tasks that contain superseded memory or concurrency assumptions

## 5. Verification

- [x] 5.1 Validate this OpenSpec change strictly
- [x] 5.2 Validate other active OpenSpec changes after alignment
- [x] 5.3 Check Markdown relative links and code-fence balance
- [x] 5.4 Check documentation for contradictory deprecated syntax or semantics
- [x] 5.5 Run formatting/diff checks and the repository local verification suite
- [x] 5.6 Review the final diff for documentation-only scope and complete all task checkboxes
