"""打ち間違いを黙って受け取らないための土台。

`ws.protectedd = True` と書いても、いままでは新しい属性が1つ付くだけで、
シートは何も変わりませんでした。書いた人は効いたと思います。数式や書式の
名前は似た物が多いので、この形の間違いは見つけるのが難しくなります。

Cell や Color は `__slots__` を持っているので前から断っていました。
Sheet・Book・Doc は持っていないので、ここで同じ形にします。

    class Sheet(NoStrayAttributes):
        _own = ("_s", "_book", "_append_row")

`_own` に書くのは、そのクラスが自分で持つ属性の名前だけです。property や
メソッドや定数は、クラスを見れば分かるので書きません。
"""
import difflib


class NoStrayAttributes:
    """知らない名前への代入を断ります。"""

    _own: tuple = ()

    def __setattr__(self, name, value):
        if name in self._own or hasattr(type(self), name):
            object.__setattr__(self, name, value)
            return
        raise AttributeError(self._shikaru(name))

    def _shikaru(self, name):
        """断りの文言。近い名前があれば添えます。"""
        aru = sorted(
            n for n in dir(type(self))
            if not n.startswith("_") and not callable(getattr(type(self), n, None))
        )
        chikai = difflib.get_close_matches(name, aru, n=1, cutoff=0.7)
        moshi = "。{} の打ち間違いではありませんか".format(chikai[0]) if chikai else ""
        return "{} に {!r} という項目はありません{}".format(
            type(self).__name__, name, moshi)
