## ADDED Requirements

### Requirement: Codegen for parallel regions and the worker pool

Native code generation SHALL emit, for a `ParallelRegion`, calls that submit the
region's chunks to the worker pool (`zirk_rt_pool_submit`) and block the calling
branch until they join (`zirk_rt_pool_join`) while the executor keeps running
other branches. Codegen SHALL emit a `zirk_rt_safepoint_poll` call at every loop
back-edge inside a region so worker threads and the executor can park for a
stop-the-world collection.

#### Scenario: region submits to the pool and joins
- **WHEN** codegen processes a `ParallelRegion`
- **THEN** the emitted code calls `zirk_rt_pool_submit` then `zirk_rt_pool_join`

#### Scenario: safepoint poll at a region back-edge
- **WHEN** codegen emits a loop inside a `parallel` region
- **THEN** a `zirk_rt_safepoint_poll` call is emitted at the loop back-edge
