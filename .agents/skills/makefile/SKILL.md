---
name: makefile
description: >-
  Best practices for authoring clean, maintainable, and portable GNU Make Makefiles. Use when
  the task involves `creating or editing Makefiles`, `Makefile`, `makefile`, `*.mk`,
  `GNUmakefile`, `GNU Make patterns`, `build automation`, or `makefile troubleshooting`.
license: MIT
metadata:
  version: "1.0.0"
---

## When to Use

- Creating a new `Makefile` for any project (C/C++, Go, Node.js, etc.)
- Reviewing or refactoring existing makefiles for correctness and maintainability
- Debugging build failures related to make rules, prerequisites, or variables
- Setting up build automation with GNU Make (pattern rules, automatic dependencies, parallel builds)
- Ensuring portability across platforms or compliance with GNU Make conventions

## Critical Patterns

- **Tabs, Not Spaces:** Every recipe line MUST start with a **tab character**. Spaces will cause a
  syntax error.
- **Declare `.PHONY`:** Always mark non-file targets (e.g., `clean`, `all`, `test`, `install`) with
  `.PHONY` to avoid conflicts with files of the same name.
- **Use `:=` Over `=` When Possible:** Prefer simple expansion (`:=`) for variables to avoid
  unexpected recursive expansion and improve performance.
- **Automatic Variables:** Use `$@` (target), `$<` (first prerequisite), `$^` (all prerequisites),
  `$?` (newer prerequisites), and `$*` (stem) in recipes to keep rules generic and reusable.
- **Use `$(wildcard ...)` Not `$(shell ls ...)`:** Never shell out for file lists; use Make's
  built-in `$(wildcard *.c)` function instead.
- **Auto-Generate Dependencies:** Use compiler flags like `-MMD -MP` to generate `.d` files rather
  than maintaining header dependencies manually.
- **Clean Target:** Always provide a `clean` target and prefix destructive commands with `-` to
  ignore errors if files don't exist.
- **Variables First, Rules Second:** Define all variables at the top, followed by the default goal,
  then build rules, then phony targets.
- **Avoid Recursive Make:** Don't use `$(MAKE) -C subdir` unless absolutely necessary; prefer
  includes or non-recursive approaches.

## File Structure

```makefile
# -------------------------------------------------------
# Project: <name>
#
# Usage:
#   make          - Build the project
#   make clean    - Remove generated files
#   make install  - Install to $(PREFIX)
#   make test     - Run tests
#
# Variables:
#   CC       - C compiler (default: gcc)
#   PREFIX   - Installation prefix (default: /usr/local)
# -------------------------------------------------------

# --- Variables ---
CC       ?= gcc
CFLAGS   := -Wall -Wextra -O2
PREFIX   ?= /usr/local

sources  := $(wildcard src/*.c)
objects  := $(sources:.c=.o)
deps     := $(objects:.o=.d)

# --- Default Goal ---
all: program

# --- Build Rules ---
program: $(objects)
	$(CC) -o $@ $^ $(LDFLAGS)

%.o: %.c
	$(CC) $(CFLAGS) -MMD -MP -c $< -o $@

-include $(deps)

# --- Phony Targets ---
.PHONY: all clean install test

clean:
	-rm -f program $(objects) $(deps)

install: program
	install -d $(PREFIX)/bin
	install -m 755 program $(PREFIX)/bin

test: program
	./run-tests.sh
```

## Code Examples

### Variables and Substitution

```makefile
# Simple expansion (evaluates immediately) - PREFERRED
CC := gcc

# Recursive expansion (evaluates when used)
CFLAGS = -Wall $(EXTRA_FLAGS)

# Conditional assignment (set default, allow override)
PREFIX ?= /usr/local

# Append to variable
CFLAGS += -g

# Always reference with $() - not bare $VAR
objects := $(sources:.c=.o)
```

### Pattern Rules and Implicit Rules

```makefile
# Custom pattern rule for generic transformations
%.pdf: %.md
	pandoc $< -o $@

# Compile with auto-dependency generation
%.o: %.c
	$(CC) $(CFLAGS) -MMD -MP -c $< -o $@
```

### Order-Only Prerequisites

```makefile
# The directory is created before compiling but does NOT
# trigger recompilation when its timestamp changes.
obj/%.o: %.c | obj
	$(CC) $(CFLAGS) -c $< -o $@

obj:
	mkdir -p obj
```

### Conditional Directives

```makefile
ifeq ($(OS),Windows_NT)
    EXE_EXT := .exe
else
    EXE_EXT :=
endif

program: main.o
	$(CC) -o program$(EXE_EXT) main.o
```

### Splitting Long Lines

```makefile
# Use backslash-newline to split for readability
sources := main.c \
           utils.c \
           parser.c \
           handler.c

# In recipes, backslash-newline continues the shell command
install: program
	install -d $(PREFIX)/bin && \
		install -m 755 program $(PREFIX)/bin
```

### Including Other Makefiles

```makefile
# Include shared configuration
include config.mk

# Include optional local overrides (no error if missing)
-include local.mk
```

### Error Handling and Validation

```makefile
# Validate required tools
ifeq ($(shell which gcc 2>/dev/null),)
    $(error gcc is not installed or not in PATH)
endif

# Validate required variables
ifndef VERSION
    $(error VERSION is not defined. Usage: make VERSION=1.2.3)
endif
```

### Special Targets

```makefile
# Delete targets if recipe fails (recommended)
.DELETE_ON_ERROR:

# Don't delete intermediate files
.SECONDARY:

# Preserve specific intermediate files
.PRECIOUS: %.o
```

### Managing Multiple Programs

```makefile
programs := prog1 prog2 prog3

.PHONY: all clean

all: $(programs)

prog1: prog1.o common.o
	$(CC) -o $@ $^

prog2: prog2.o common.o
	$(CC) -o $@ $^

clean:
	-rm -f $(programs) *.o
```

## Anti-Patterns to Avoid

| Anti-Pattern                       | Why It's Bad                                     | Do Instead                          |
|------------------------------------|--------------------------------------------------|-------------------------------------|
| Spaces instead of tabs in recipes  | Syntax error, hard to debug                      | Use **tab** characters              |
| `$(shell ls *.c)` for file lists   | Creates subprocess, fragile                      | `$(wildcard *.c)`                   |
| Missing `.PHONY` declarations      | `clean` won't run if a file named `clean` exists | `.PHONY: clean`                     |
| Hardcoded file lists               | Maintenance burden, easy to forget new files     | `$(wildcard ...)` or auto-discovery |
| Complex shell scripts in recipes   | Hard to read and debug                           | Move to a separate `.sh` script     |
| Recursive make `$(MAKE) -C subdir` | Breaks dependency tracking across dirs           | `include` or non-recursive make     |
| Using `=` everywhere               | Unexpected recursive expansion, slower           | Use `:=` for simple values          |
| Circular dependencies              | Infinite loop, build failure                     | Restructure dependency graph        |

## Commands

```bash
# Dry run - see what would execute without running
make -n

# Force rebuild all targets
make -B

# Parallel build (use number of CPU cores)
make -j$(nproc)

# Print the database of rules and variables (debugging)
make -p

# Show which makefile and line number each rule comes from
make -d

# Override a variable from the command line
make CC=clang CFLAGS="-O3"
```

## Resources

- [GNU Make Manual](https://www.gnu.org/software/make/manual/) - The authoritative reference
