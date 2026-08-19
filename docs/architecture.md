# Architecture

Dependency flow is one-way:

```text
app -> ui -> gpui + gpui-component
app -> rendering -> pdf-engine -> document-core
app -> pdf-engine-zpdf -> pdf-engine + zpdf
test-support -> generated fixture bytes
```

- `document-core` owns stable IDs, capabilities, PDF geometry, and viewport transforms.
- `pdf-engine` owns backend-independent read and render contracts.
- `pdf-engine-zpdf` converts zpdf values and errors at one boundary.
- `rendering` owns document worker lifecycle and render generations.
- `ui` owns GPUI elements and has no PDF-library dependency.
- `app` composes the worker and `Root` window.

Mutable engine state belongs to one worker. Stale render generations are discarded before delivery. Explicit shutdown joins the worker; dropping a handle requests shutdown without blocking the UI thread.

