"""`openpyxl.chart` と同じ名前。中身は `officework.chart` の openpyxl 形の
クラスです。officework に無い図の種類の名前はありません(その所は止まります)。"""
from .. import chart as _chart

__all__ = []
for _name in ("BarChart", "LineChart", "PieChart", "DoughnutChart", "AreaChart",
              "ScatterChart", "RadarChart", "StockChart", "SurfaceChart",
              "Reference", "Series"):
    if hasattr(_chart, _name):
        globals()[_name] = getattr(_chart, _name)
        __all__.append(_name)
del _name
