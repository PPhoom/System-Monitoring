# System Monitoring — Architecture

## State Model
The system monitoring frontend (Rust + Iced) is modeled as a reactive state machine:

- **Application State**
  - `DashboardState` (active view: metrics, logs, alerts)
  - `MetricsState` (current queries, cached results, time range)
  - `LogsState` (query filters, scroll position, retrieved entries)
  - `AlertsState` (active alerts, silenced alerts, filter criteria)
  - `ConfigState` (API endpoints, auth tokens, refresh intervals)

- **State Transitions**
  - Triggered by incoming messages (user actions, API responses, errors)
  - Immutable update model: new state produced on every transition

---

## Message Flow
The system uses an event-driven message passing pattern:

1. **User Input → Message**
   - Example: user clicks "Refresh" → `Message::RefreshMetrics`

2. **Command Dispatch**
   - `update` function processes message
   - Async command created for API call (e.g., Prometheus query)

3. **External API Call → Response Message**
   - On completion, returns a new message
   - Example: `Message::MetricsLoaded(Result<MetricData, ApiError>)`

4. **State Update**
   - Application updates state depending on message
   - UI re-renders based on updated state

**Flow Example:**
```
User clicks Refresh
   ↓
Message::RefreshMetrics
   ↓ (update creates async Command)
Prometheus API request
   ↓
Message::MetricsLoaded(Result<MetricData, ApiError>)
   ↓
State updated → UI reflects new metrics or error
```

---

## Error Handling Strategy
Robust error handling is essential for monitoring reliability.

- **Error Categories**
  - **Network Errors**: API unreachable, timeout, connection reset
  - **Parsing Errors**: Invalid JSON or unexpected schema
  - **Application Errors**: State inconsistency, unhandled message

- **Error Propagation**
  - All async results wrapped in `Result<T, ApiError>`
  - Errors converted into messages (e.g., `Message::ApiError(ApiError)`) to be handled in `update`

- **User Feedback**
  - Display inline error messages in the UI (e.g., "Failed to fetch metrics")
  - Retry options exposed via buttons ("Retry" → re-dispatch command)
  - Silent background retries with exponential backoff for transient errors

- **Resilience**
  - Maintain last known good state (cached metrics/logs) if new fetch fails
  - Use circuit breaker approach for repeated failures to avoid UI lockups
  - Log errors locally for diagnostics