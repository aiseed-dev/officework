"""打ち間違いを黙って受け取らないための土台。

`ws.protectedd = True` と書いても、いままでは新しい属性が1つ付くだけで、
シートは何も変わりませんでした。書いた人は効いたと思います。数式や書式の
名前は似た物が多いので、この形の間違いは見つけるのが難しくなります。

Cell や Color は `__slots__` を持っているので前から断っていました。
Sheet・Book・Doc は持っていないので、ここで同じ形にします。

    class Sheet(NoStrayAttributes):
        _own = ("_s", "_book", "_append_row")
        _engine_attr = "_s"

`_own` に書くのは、そのクラスが自分で持つ属性の名前だけです。
`_engine_attr` は包んでいるエンジンの物の置き場で、そこにある名前への代入は
通します(`ws.paper_size = "9"` はエンジンの setter に届きます)。property や
メソッドや定数は、クラスを見れば分かるので書きません。
"""
import difflib


class NoStrayAttributes:
    """知らない名前への代入を断ります。"""

    _own: tuple = ()
    # 包んでいるエンジンの物の置き場(`_s` / `_b` / `_d`)。エンジンの側に
    # ある名前(`ws.paper_size` など)への代入は、そのまま通します。
    # 読む側は `__getattr__` が前から通していたのに、代入だけ断っていたので、
    # `ws.paper_size = ws.PAPERSIZE_A4` が「項目はありません」で止まりました
    # (2026-09-08、受け入れ試験の5枚を作り直して見つけた)
    _engine_attr: str = ""

    def __setattr__(self, name, value):
        if name in self._own or hasattr(type(self), name):
            object.__setattr__(self, name, value)
            return
        raw = self.__dict__.get(self._engine_attr) if self._engine_attr else None
        if raw is not None and not name.startswith("_") and hasattr(type(raw), name):
            setattr(raw, name, value)
            return
        raise AttributeError(self._shikaru(name))

    def _shikaru(self, name):
        """断りの文言。近い名前があれば添えます。"""
        raw = self.__dict__.get(self._engine_attr) if self._engine_attr else None
        aru = sorted(
            set(
                n for n in dir(type(self))
                if not n.startswith("_") and not callable(getattr(type(self), n, None))
            )
            | set(
                n for n in (dir(type(raw)) if raw is not None else ())
                if not n.startswith("_") and not callable(getattr(type(raw), n, None))
            )
        )
        chikai = difflib.get_close_matches(name, aru, n=1, cutoff=0.7)
        moshi = "。{} の打ち間違いではありませんか".format(chikai[0]) if chikai else ""
        return "{} に {!r} という項目はありません{}".format(
            type(self).__name__, name, moshi)
