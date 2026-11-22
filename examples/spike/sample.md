# Renderer spike

Vitrine reads **Markdown artifacts** from coding agents. This file checks prose, a table,
a normal code block, a local image and several Mermaid diagram types. See the
[spec](../../Vitrine-MVP.md) for details.

## Table

| Step | Owner  | Status      |
|------|:------:|------------:|
| Scan | Rust   | done        |
| Read | Rust   | in progress |
| Draw | Rune   | planned     |

## Code

```rust
fn main() {
    let greeting = "hello, vitrine";
    println!("{greeting}");
}
```

## Image

![Rune tree component](tree.png)

## Flowchart

```mermaid
flowchart LR
    A[Open folder] --> B{Markdown found?}
    B -->|yes| C[Show tree]
    B -->|no| D[Empty state]
    C --> E[Read file]
```

## Sequence

```mermaid
sequenceDiagram
    participant U as User
    participant V as Vitrine
    participant G as git
    U->>V: Press Cmd+R
    V->>V: Rescan folder
    V->>G: status --porcelain=v2
    G-->>V: staged + unstaged
    V-->>U: Updated preview
```

## Class

```mermaid
classDiagram
    class Project {
        +PathBuf path
        +String name
        +open()
    }
    class Document {
        +PathBuf path
        +read() String
    }
    Project "1" --> "*" Document
```

## State

```mermaid
stateDiagram-v2
    [*] --> Empty
    Empty --> Selected: pick file
    Selected --> Missing: refresh, file gone
    Selected --> ReadError: refresh, unreadable
    Missing --> Selected: pick file
    ReadError --> Selected: retry
```

## Entity relationship

```mermaid
erDiagram
    PROJECT ||--o{ DOCUMENT : contains
    DOCUMENT ||--o{ DIAGRAM : embeds
    PROJECT {
        string path
        string name
    }
```

## Broken diagram

```mermaid
flowchart LR
    A --> 
    B[[[
```
