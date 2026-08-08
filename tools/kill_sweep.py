#!/usr/bin/env python3
"""点検で立てた calc だけを落とす。

`pkill -f release/calc` は**発注者が開いている窓まで殺す**。
点検の道具は立てた pid を控えているので、その分だけを落とす。
"""
import importlib.util, os, sys
spec = importlib.util.spec_from_file_location(
    "rs", os.path.join(os.path.dirname(os.path.abspath(__file__)), "ribbon_sweep.py"))
rs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rs)
print(f"落とした点検用の calc: {rs.App.kill_strays()} 件")
