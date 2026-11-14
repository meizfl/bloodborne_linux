// SPDX-License-Identifier: GPL-2.0-or-later
// Linux primitives used by the offline glyph hooks; no game or GPU required.
#include "lc_platform.h"

#include <cassert>
#include <cstdint>
#include <cstring>
#include <sys/mman.h>
#include <unistd.h>

int main() {
    const std::uint32_t value = 0x12345678;
    std::uint32_t copy = 0;
    assert(lc_platform::read(&copy, reinterpret_cast<std::uintptr_t>(&value), sizeof(copy)));
    assert(copy == value);
    assert(!lc_platform::read(&copy, 0, sizeof(copy)));

    const auto page = static_cast<std::size_t>(sysconf(_SC_PAGESIZE));
    auto* code = lc_platform::alloc_code(page);
    assert(code);
    // mov eax, 42; ret (x86-64, the architecture required by the loader).
    const unsigned char initial[] = {0xb8, 42, 0, 0, 0, 0xc3};
    std::memcpy(code, initial, sizeof(initial));
    assert(lc_platform::seal_code(code, page));
    auto invoke = reinterpret_cast<int (*)()>(code);
    assert(invoke() == 42);
    const unsigned char replacement[] = {0xb8, 7, 0, 0, 0, 0xc3};
    assert(lc_platform::write_code(code, replacement, sizeof(replacement)));
    assert(invoke() == 7);
    assert(munmap(code, page) == 0);
    assert(!lc_platform::read(&copy, reinterpret_cast<std::uintptr_t>(code), sizeof(copy)));
}
