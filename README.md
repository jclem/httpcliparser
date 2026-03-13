# httpcliparser

`httpcliparser` parses HTTP CLI input strings into request headers, query params,
and JSON body data.

It is extracted from [`get`](https://github.com/jclem/get) so the parsing logic
can be reused independently in CLIs, test helpers, and other request-building
tools.

## Install

```bash
cargo add httpcliparser
```

## Example

```rust
use httpcliparser::{parse_input, ParsedHeader, ParsedInput, ParsedQueryParam};
use serde_json::json;

let parsed = parse_input([
    "Authorization:Bearer token",
    "q==hello world",
    "foo[bar]=baz",
    "is_draft:=true",
])?;

assert_eq!(
    parsed,
    ParsedInput {
        headers: vec![ParsedHeader {
            name: "Authorization".into(),
            value: "Bearer token".into(),
        }],
        query_params: vec![ParsedQueryParam {
            name: "q".into(),
            value: "hello world".into(),
        }],
        body: Some(json!({
            "foo": { "bar": "baz" },
            "is_draft": true,
        })),
    }
);
# Ok::<(), httpcliparser::ParseInputError>(())
```

## Supported Syntax

Each input part is parsed independently using this precedence order:

1. `path:=json`
2. `name==value`
3. `Header:Value`
4. `path=value`

If no body assignments are present, `ParsedInput.body` is `None`.

| Form | Meaning | Example |
| --- | --- | --- |
| `path:=json` | Body assignment with typed JSON parsing | `count:=2` |
| `name==value` | Query parameter | `page==2` |
| `Header:Value` | Header | `Accept:application/json` |
| `path=value` | Body assignment with a raw string value | `title=hello` |

### Header Syntax

`Header:Value`

- Header names allow ASCII letters, numbers, `-`, and `_`.
- Header values may contain additional `:` characters.
- Values may contain spaces.
- Repeated headers are preserved in input order.

Examples:

```text
Accept:application/json
X-Trace-Id:abc-123
Authorization:Bearer token
```

### Query Param Syntax

`name==value`

- Query names can contain any character except `=`.
- Query names must not be empty.
- Empty values are allowed.
- Names and values may contain spaces.
- Repeated keys are preserved in order.

Examples:

```text
q==rust
page==2
foo bar==baz qux
tag==
tag==parser
```

### Body Assignment Syntax

`path=value` stores a JSON string.

`path:=json` parses the right-hand side as JSON, so booleans, numbers, arrays,
objects, and `null` are preserved.

Examples:

```text
title=hello
enabled:=true
count:=2
labels:=["rust","cli"]
meta:={"owner":"jonathan"}
foo:="string"
foo:=null
```

String assignments always stay strings:

```text
enabled=true
count=2
```

becomes:

```json
{
  "enabled": "true",
  "count": "2"
}
```

### Path Syntax

Body paths are built from segments. The parser supports all of these segment
forms:

| Segment form | Meaning | Example |
| --- | --- | --- |
| `foo` | Bare object key | `user=name` |
| `.foo` | Dotted object key | `user.name=alex` |
| `[foo]` | Bracket object key | `user[name]=alex` |
| `0` | Bare array index | `0=first` |
| `.0` | Dotted array index | `items.0=first` |
| `[0]` | Bracket array index | `items[0]=first` |
| `[]` | Append to array | `items[]=next` |

Important path rules:

- Bare and dotted numeric segments are treated as array indexes.
- Bracket keys allow characters that plain keys do not, such as `.` in
  `meta[build.version]=1`.
- Plain object keys stop at `.`, `[`, `:`, and `=`.
- Bracket keys continue until the next `]`.
- Segments can be mixed freely across objects and arrays.
- Separators are flexible, so forms like `foo[].bar=baz`, `foo[]bar=baz`, and
  `foo.3[][]a[]4[].b[c][][d]=x` are all valid.

Examples:

```text
project.name=apollo
project[build.version]=v1
items[]=a
items[]=b
items[2]=third
items.0=first
root[0].user[name]=alex
[]=first
[]=second
```

### Body Construction Semantics

- Missing objects and arrays are created automatically while traversing a path.
- Sparse array indexes are padded with `null`.
- Reassigning the same path overwrites the previous value.
- Repeating `[]` appends new values in order.
- `null` placeholders can later become objects or arrays if a deeper path needs
  them.
- If traversal hits an existing non-container value, parsing fails with a type
  mismatch error.

Examples:

```text
foo[3]=bar
```

becomes:

```json
{
  "foo": [null, null, null, "bar"]
}
```

```text
foo=bar
foo=baz
```

becomes:

```json
{
  "foo": "baz"
}
```

```text
foo:=null
foo[bar]=baz
```

becomes:

```json
{
  "foo": {
    "bar": "baz"
  }
}
```

### Mixed Inputs

Headers, query params, and body assignments can all be mixed in one parse call:

```rust
use httpcliparser::{parse_input, ParsedHeader, ParsedInput, ParsedQueryParam};
use serde_json::json;

let parsed = parse_input([
    "Accept:application/json",
    "expand==owner",
    "expand==labels",
    "title=write-readme",
    "priority:=2",
    "meta[tags][]=docs",
])?;

assert_eq!(
    parsed,
    ParsedInput {
        headers: vec![ParsedHeader {
            name: "Accept".into(),
            value: "application/json".into(),
        }],
        query_params: vec![
            ParsedQueryParam {
                name: "expand".into(),
                value: "owner".into(),
            },
            ParsedQueryParam {
                name: "expand".into(),
                value: "labels".into(),
            },
        ],
        body: Some(json!({
            "title": "write-readme",
            "priority": 2,
            "meta": {
                "tags": ["docs"]
            }
        })),
    }
);
# Ok::<(), httpcliparser::ParseInputError>(())
```

### Error Behavior

The parser reports three kinds of failures:

- `unexpected input: ...` for invalid headers, query params, or path syntax.
- `invalid JSON value in ...` when a `:=` assignment is not valid JSON.
- Type mismatch errors when a later path tries to traverse through a scalar
  value as if it were an object or array.

## Publishing

This repo includes a GitHub Actions publish workflow.

To enable it:

1. Create a crates.io API token.
2. Add it to the repository secrets as `CARGO_REGISTRY_TOKEN`.
3. Bump `version` in `Cargo.toml`.
4. Push a matching tag like `v0.1.0`.

The publish workflow reruns formatting, linting, tests, verifies the tag matches
the crate version, packages the crate, and then publishes it to crates.io.
