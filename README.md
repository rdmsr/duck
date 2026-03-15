```
quack! >o)
	   (_>
       duck
```

> Modern documentation tool for C and C++ projects - inspired by `rustdoc`.


**[Live demo](https://rdmsr.github.io/duck)**

## What is duck?
Most C/C++ documentation generators (like Doxygen) produce API references, a list of functions and types. `duck` goes further: it lets you write structured, book-style guides *alongside* your API reference, integrated directly with your codebase.


## Features
- **`rustdoc`-style documentation comments**, written in Markdown
- **Documentation tests** - code blocks in your docs run as real tests 
- **`mdbook`-style hierarchy** for page structure and navigation
- **[Mermaid](https://mermaid.js.org)** support for graphs
- **Customizable themes** with user-supplied CSS and Sublime Text syntax highlighting themes
- **libclang-based parser** with support for records, enums, functions and namespaces
- **Fast**: benchmarked to run around **4-5x faster** than Doxygen with clang-assisted parsing


## Usage
See [USAGE.md](USAGE.md)

## Projects using duck
Open a PR to add your project!

- [uACPI](https://uacpi.github.io/)
