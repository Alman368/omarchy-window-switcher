from __future__ import annotations

import importlib.machinery
import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "bin" / "omarchy-window-switcher"
loader = importlib.machinery.SourceFileLoader("switcher", str(MODULE_PATH))
spec = importlib.util.spec_from_loader(loader.name, loader)
switcher = importlib.util.module_from_spec(spec)
sys.modules[loader.name] = switcher
loader.exec_module(switcher)


CLIENTS = [
    {
        "address": "0x1",
        "mapped": True,
        "hidden": False,
        "acceptsInput": True,
        "workspace": {"id": 1, "name": "1"},
        "monitor": 0,
        "class": "zen",
        "title": "Browser",
        "focusHistoryID": 0,
    },
    {
        "address": "0x2",
        "mapped": True,
        "hidden": False,
        "acceptsInput": True,
        "workspace": {"id": 2, "name": "2"},
        "monitor": 0,
        "class": "code",
        "title": "Editor",
        "focusHistoryID": 1,
    },
    {
        "address": "0x3",
        "mapped": True,
        "hidden": False,
        "acceptsInput": True,
        "workspace": {"id": 3, "name": "3"},
        "monitor": 1,
        "class": "zed",
        "title": "Project",
        "focusHistoryID": 2,
    },
    {
        "address": "0x4",
        "mapped": True,
        "hidden": False,
        "acceptsInput": True,
        "workspace": {"id": -98, "name": "special:scratchpad"},
        "monitor": 0,
        "class": "scratchpad",
        "title": "Scratchpad",
        "focusHistoryID": 3,
    },
]


def test_filter_defaults_skip_special_and_other_monitor():
    windows = switcher.parse_clients(CLIENTS)
    filtered = switcher.filter_windows(windows, current_monitor=0)

    assert [window.address for window in filtered] == ["0x1", "0x2"]


def test_filter_all_monitors_keeps_focus_order():
    windows = switcher.parse_clients(CLIENTS)
    filtered = switcher.filter_windows(windows, current_monitor=None)

    assert [window.address for window in filtered] == ["0x1", "0x2", "0x3"]


def test_next_window_skips_active():
    windows = switcher.filter_windows(switcher.parse_clients(CLIENTS), current_monitor=None)

    assert switcher.next_window(windows, "0x1").address == "0x2"


def test_focus_commands_switch_workspace_before_focus():
    window = switcher.parse_clients(CLIENTS)[1]

    assert switcher.focus_commands(window) == [
        ["hyprctl", "dispatch", "workspace", "2"],
        ["hyprctl", "dispatch", "focuswindow", "address:0x2"],
        ["hyprctl", "dispatch", "bringactivetotop"],
    ]
