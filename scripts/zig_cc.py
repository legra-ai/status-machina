#!/usr/bin/env python3
"""Translate GNU driver arguments into Zig's supported command line."""

import os
import sys


# GNU linker flags Rust passes that Zig's driver rejects.
UNSUPPORTED = (
    # Rust adds this to optimized GNU targets. Zig has no corresponding option
    # (ziglang/zig src/main.zig linker_optimization); LLVM code generation and
    # LTO optimization arguments are preserved.
    '-Wl,-O1',
    # Rust adds this to every aarch64 GNU/Linux link. Zig reports it as an
    # unsupported linker argument; cargo-zigbuild, which links the aarch64
    # wheel, removes it the same way, so the wheel and the addon link alike.
    '-Wl,--fix-cortex-a53-843419',
)


def translate(arguments: list[str]) -> list[str]:
    """Remove the GNU linker flags Zig diagnoses from compiler-driver calls."""
    if arguments and arguments[0] in ('cc', 'c++'):
        return [argument for argument in arguments if argument not in UNSUPPORTED]
    return arguments


if __name__ == '__main__':
    os.execvp('zig', ['zig', *translate(sys.argv[1:])])
