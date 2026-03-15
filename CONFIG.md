# Configuration options
The configuration file is a TOML file named `duck.toml` with the following sections:

## `project`
- `name`: Project name.
- `version`: Project version.

## `input`
- `glob`: Glob to use for finding documented files.
- `compiler_arguments`: List of arguments to pass to libclang while parsing code.

## `output`
- `static_dir`: Path to a directory containing static files that will be copied in the output directory, this is where `style.css` will typically be located.
- `path`: Path to the output directory.
- `root_namespace` (optional): Namespace to use as the root, this is useful for libraries that only globally expose one namespace and want the index to be based on that namespace.
- `base_url`: Base URL to prepend all paths with (optional).
- `enable_mermaid`: Whether to enable mermaid, default: `true`
- `bundle_mermaid`: Whether to bundle mermaid (`mermaid.mjs` in the static directory), otherwise it is fetched from a release online, default: `false`
- `bundle_minisearch`: Whether to bundle minisearch (`minisearch.js` in the static directory), otherwise it is fetched from a release online, default `false`
- `theme` (optional): Syntax highlighting theme to use, this must be part of the theme set specified by `theme_set_file` or the [default theme set](https://github.com/getzola/zola/tree/master/components/config/sublime/themes)
- `theme_file` (optional): Path to a sublime text .tmTheme syntax highlighting theme to use
- `theme_set_file` (optional): Path to a compressed .themedump `syntect` theme set file


## `pages` (optional)
- `index` (optional): Markdown file to use as the index file, if an index page is not specified, the root namespace's comment will be used instead.
- `book` (optional): Path to directory containing an `mdbook`-type `SUMMARY.md` file, listing all pages

## `doctests` (optional)
- `enable`: Whether to enable documentation tests or not.
- `run`: Whether to run documentation tests or not (if disabled, tests will only be compiled).
- `compiler_invocation`: Compiler invocation to use to compile documentation tests, this is represented as an array containing `argv`. The sentinel values `{file}` and `{out}` are replaced at runtime by the appropriate values.


# Example

```toml
[project]
name = "Example"
version = "0.1.0"

[input]
glob = "include/**/*.hpp"
compiler_arguments = ["-Iinclude", "-std=gnu++20", "-xc++"]

[pages]
index = "README.md"
book = "extra-pages"

[output]
static_dir = "static"
path = "docs"
base_url = "/duck"
theme = "ayu-dark"

[doctests]
enable = false 
run = true
compiler_invocation = ["clang++", "{file}", "-o", "{out}", "-Iinclude", "-std=c++20"]
```
