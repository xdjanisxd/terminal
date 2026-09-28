"""Verify the packaged Windows executable embeds the committed ICO, without launching it."""

import ctypes
from ctypes import wintypes
from pathlib import Path
import struct


def validate_windows_icon(binary: Path) -> None:
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
    kernel.LoadLibraryExW.restype = wintypes.HMODULE
    kernel.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
    kernel.FindResourceW.restype = ctypes.c_void_p
    kernel.SizeofResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
    kernel.SizeofResource.restype = wintypes.DWORD
    kernel.LoadResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
    kernel.LoadResource.restype = ctypes.c_void_p
    kernel.LockResource.argtypes = [ctypes.c_void_p]
    kernel.LockResource.restype = ctypes.c_void_p
    kernel.FreeLibrary.argtypes = [wintypes.HMODULE]
    kernel.FreeLibrary.restype = wintypes.BOOL
    # LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE: never execute code.
    module = kernel.LoadLibraryExW(str(binary.resolve()), None, 0x22)
    if not module:
        raise ctypes.WinError(ctypes.get_last_error())

    def resource(kind: int, identifier: int) -> bytes:
        found = kernel.FindResourceW(module, identifier, kind)
        if not found:
            raise RuntimeError(f"missing executable resource: type={kind}, id={identifier}")
        size = kernel.SizeofResource(module, found)
        pointer = kernel.LockResource(kernel.LoadResource(module, found))
        if not pointer or not size:
            raise RuntimeError("empty or unreadable executable resource")
        return ctypes.string_at(pointer, size)

    try:
        ico = (Path(__file__).resolve().parent.parent /
               "assets/branding/windows/terminal.ico").read_bytes()
        group = resource(14, 1)  # RT_GROUP_ICON, matching winresource and winit.
        if group[:6] != ico[:6]:
            raise RuntimeError("embedded icon header differs from canonical ICO")
        count = struct.unpack_from("<H", ico, 4)[0]
        if len(group) != 6 + count * 14:
            raise RuntimeError("invalid embedded icon group length")
        for index in range(count):
            source = 6 + index * 16
            embedded = 6 + index * 14
            source_entry = struct.unpack_from("<BBBBHHI", ico, source)
            embedded_entry = struct.unpack_from("<BBBBHHI", group, embedded)
            # rc.exe normalizes Pillow's unspecified PNG color planes (0) to 1.
            expected_entry = (*source_entry[:4], source_entry[4] or 1, *source_entry[5:])
            if expected_entry != embedded_entry:
                raise RuntimeError("embedded icon dimensions differ from canonical ICO")
            length, offset = struct.unpack_from("<II", ico, source + 8)
            identifier = struct.unpack_from("<H", group, embedded + 12)[0]
            if resource(3, identifier) != ico[offset:offset + length]:
                raise RuntimeError("embedded icon pixels differ from canonical ICO")
    finally:
        kernel.FreeLibrary(module)
