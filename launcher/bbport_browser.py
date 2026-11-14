# SPDX-License-Identifier: GPL-2.0-or-later
"""Open links through the desktop default while retaining Windows behavior."""
import sys
import webbrowser
import bbport_platform as host


def open(url, new=0, autoraise=True):
    if sys.platform == 'win32':
        return webbrowser.open(url, new=new, autoraise=autoraise)
    return host.open_url(url)
