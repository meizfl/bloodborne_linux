#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Check actual Tk 9.2 Wayland framebuffer pixels after partial Canvas redraws.

Run with check-tk92-wayland.sh inside the experimental toolchain container.
"""
import ctypes
import time
import tkinter as tk


class XImagePrefix(ctypes.Structure):
    _fields_ = [('width', ctypes.c_int), ('height', ctypes.c_int),
                ('xoffset', ctypes.c_int), ('format', ctypes.c_int),
                ('data', ctypes.c_void_p)]


def main():
    root = tk.Tk()
    try:
        assert root.tk.call('tk', 'windowingsystem') == 'wayland'
        library = ctypes.CDLL('libtcl9tk9.2.so')
        library.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int,
                                     ctypes.c_int, ctypes.c_uint, ctypes.c_uint,
                                     ctypes.c_ulong, ctypes.c_int]
        library.XGetImage.restype = ctypes.POINTER(XImagePrefix)
        library.XDestroyImage.argtypes = [ctypes.POINTER(XImagePrefix)]
        library.XDestroyImage.restype = ctypes.c_int
        canvas = tk.Canvas(root, width=400, height=220, bg='black', highlightthickness=0)
        canvas.pack()
        photo = tk.PhotoImage(width=400, height=220)
        photo.put('#356789', to=(0, 0, 400, 220))
        canvas.create_image(0, 0, image=photo, anchor='nw')
        root.update()
        time.sleep(.1)
        root.update()

        def pixel(x, y):
            # Tk's Wayland XGetImage reads its backing FBO, not the source photo.
            image = library.XGetImage(None, canvas.winfo_id(), x, y, 1, 1,
                                      ctypes.c_ulong(-1), 2)
            assert image, 'Cannot read Canvas framebuffer'
            try:
                return ctypes.c_uint32.from_address(image.contents.data).value & 0xffffff
            finally:
                library.XDestroyImage(image)

        for x, y, width, height in ((40, 40, 80, 50), (20, 90, 330, 12), (170, 20, 25, 150)):
            item = canvas.create_rectangle(x, y, x + width, y + height, fill='white', outline='white')
            root.update()
            canvas.delete(item)
            root.update()
            for px, py in ((x - 4, y - 4), (x + width + 4, y - 4),
                           (x - 4, y + height + 4), (x + width + 4, y + height + 4)):
                actual = pixel(px, py)
                assert actual == 0x356789, f'Damage margin at {(px, py)}: {actual:06x}'
        print('PASS: framebuffer pixels preserved around partial redraws of several shapes')
    finally:
        root.destroy()


if __name__ == '__main__':
    main()
