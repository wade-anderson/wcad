# WCAD: Requirements Specification
## Modern 2D CAD for the Linux Desktop

### 1. Vision and Scope
WCAD is a high-performance, open-source 2D Computer-Aided Design (CAD) application designed specifically for the Linux hobbyist ecosystem. It aims to bridge the gap between overly simplistic drawing tools and professional-grade, yet dated, CAD software.

#### 1.1 Target Audience
- **Makers & DIYers**: Designing parts for 3D printing, laser cutting, or CNC machining.
- **Woodworkers**: Creating plans for furniture and home projects.
- **Hobbyist Architects**: Sketching floor plans or landscaping ideas.
- **Open Source Enthusiasts**: Users who want a modern, GTK4-based native experience on Linux.

#### 1.2 Core Goals
- **Premium UX**: A modern, clean interface using Libadwaita that feels at home on a GNOME desktop.
- **Extreme Performance**: Leveraging Rust and hardware acceleration (Wgpu) to handle complex 2D drawings with thousands of entities without lag.
- **Simplicity vs. Power**: Intuitive enough for a beginner to start drafting in minutes, but powerful enough for complex mechanical layouts.
- **Hackability**: A first-class plugin system and a clean, documented architecture.

---

### 2. Functional Requirements

#### 2.1 Geometric Entities
The application must support the following core entities:
- **Primitives**: Points, Lines, Circles, Arcs, Ellipses.
- **Complex Entities**: Polylines (with arc segments), Splines (NURBS-based), and Hatch patterns.
- **Annotations**: Single-line and multi-line text, Dimensions (Linear, Aligned, Radial, Angular).
- **Images**: Reference image overlays with transparency and scaling.

#### 2.2 Drafting & Precision
- **Coordinate Systems**: Absolute, Relative, and Polar coordinates.
- **Object Snapping (OSNAP)**: Endpoint, Midpoint, Center, Intersection, Perpendicular, Tangent, and Grid Snap.
- **Constraints**: Basic 2D geometric constraints (Parallel, Perpendicular, Tangent, Coincident).
- **Unit Support**: Metric (mm, cm, m) and Imperial (in, ft, fractional representation).

#### 2.3 Layer Management
- Hierarchical layer organization.
- Visibility, Locking, and Printability toggles per layer.
- Override attributes (Color, Linetype, Lineweight) at the layer level.

#### 2.4 Modification Tools
- **Standard Edits**: Move, Copy, Rotate, Scale, Mirror.
- **Geometry Logic**: Trim, Extend, Offset, Fillet, Chamfer.
- **Organization**: Grouping/Ungrouping and Block/Symbol creation for reusability.

#### 2.5 Data I/O
- **Native Format**: A human-readable JSON-based format (or a lean binary format with a JSON sidecar).
- **Import**: DXF (R12-R2018), SVG (for vector art conversion), and Image files.
- **Export**: DXF, SVG, PDF (Technical drawing layout), and high-resolution PNG/JPEG.

---

### 3. Technical Requirements

#### 3.1 Language & Performance
- **Language**: Rust 1.75+ for the entire stack (Core and UI).
- **Memory Safety**: Zero usage of `unsafe` in business logic; minimal and audited `unsafe` in low-level rendering if necessary.
- **Concurrency**: Utilize `rayon` for parallel geometry calculations (e.g., hatch generation, complex offsets).

#### 3.2 UI Framework
- **Toolkit**: GTK4 + Libadwaita for a native GNOME 45+ look and feel.
- **Pattern**: Relm4 or standard `gtk-rs` with a strong emphasis on reactive state management.
- **Accessibility**: Full support for screen readers and keyboard-only navigation.

#### 3.3 Rendering Engine
- **Backend**: `wgpu` for cross-platform, hardware-accelerated 2D rendering.
- **Anti-aliasing**: Implementation of high-quality MSAA or specialized 2D anti-aliasing techniques (like Lyon or Vello).
- **Shaders**: Custom WGSL shaders for dashed lines, infinite grids, and high-performance selection highlighting.

---

### 4. Architectural Principles

#### 4.1 Clean Architecture
- **Domain Layer**: Pure Rust entities and geometric logic, independent of GTK or Wgpu.
- **Infrastructure Layer**: Implementation of file I/O, rendering backends, and OS integration.
- **Application Layer**: Use cases such as "Add Entity", "Apply Constraint", "Export to PDF".

#### 4.2 Command Pattern
- Every user action must be encapsulated as a Command.
- Support for infinite Undo/Redo history.
- Ability to record and playback command sequences (Macro support).

#### 4.3 Plugin Architecture
- **Runtime**: WebAssembly (WASM) via `wasmtime` or `wasmer`.
- **API**: A stable, versioned C-ABI or WIT-based interface for external plugins to interact with the geometry engine and UI.
- **UI Extension**: Ability for plugins to add custom sidebars or tool buttons via a safe manifest.

---

### 5. Linux Desktop Integration
- **Wayland**: Full native support with proper scaling.
- **Appearance**: Respect system-wide Dark/Light mode and Accent colors via Libadwaita.
- **Portals**: Use `xdg-desktop-portal` for secure file access and printing.
- **Distribution**: Flatpak-first distribution strategy, ensuring all dependencies are bundled.
