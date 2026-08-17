# macOS の署名と公証 — 用意する物と手順

配る `.dmg` に**署名して公証する**ための下ごしらえ。ここを済ませると、
利用者は落として**そのまま開ける**(「右クリック→開く」が要らなくなる)。

出す形は **Developer ID の直配布**で、Mac App Store ではない。理由は
[docs/sekkei/ayumi.ja.md](sekkei/ayumi.ja.md) の配布の節(gpui が GPL の
3クレートを必須で引くので、Apple のストアの規約と衝突する)。

作業は**一度きり**。済んだら CI が毎回やる。

---

## 0. いま何があるか確かめる(Mac で1行)

証明書は「作った覚えがあるか」ではなく、**機械に聞く**。

```sh
security find-identity -v -p codesigning
```

`Developer ID Application: …(チーム ID)` の行が出れば**もうある** →
「2. 証明書を .p12 に書き出す」へ。出なければ「1.」から。

> `Apple Development:` や `Mac Developer:` しか出ない場合は**別物**。
> それらは手元で動かすための証明書で、配る物には使えない。

チーム ID は次でも見られる。

```sh
xcrun altool --list-providers -u <Apple ID> 2>/dev/null || true
```

見えなければ <https://developer.apple.com/account> の Membership に出ている
10 桁の英数字。

---

## 1. Developer ID Application の証明書を作る(まだ無い場合)

Apple Developer Program の加入が要る(加入済み)。

### 1-1. 署名要求(CSR)を Mac で作る

1. **キーチェーンアクセス**を開く
2. メニューの「キーチェーンアクセス」→「証明書アシスタント」→
   **「認証局に証明書を要求…」**
3. メールアドレスと通称を入れ、**「ディスクに保存」**と
   **「鍵ペア情報を指定」**にチェック
4. 鍵の情報は **2048 ビット・RSA**
5. `CertificateSigningRequest.certSigningRequest` が出来る

> ここで**秘密鍵がこの Mac の中に作られる**。以後の署名はこの鍵で行う —
> 鍵が消えると証明書も使えなくなるので、2 で書き出した `.p12` が控えになる。

### 1-2. Apple に出して受け取る

1. <https://developer.apple.com/account/resources/certificates/list>
2. **＋** →「**Developer ID Application**」を選ぶ
3. 1-1 の `.certSigningRequest` を上げる
4. 出来た `.cer` を落として**ダブルクリック**(キーチェーンに入る)
5. `security find-identity -v -p codesigning` で出ることを確かめる

---

## 2. 証明書を .p12 に書き出す

1. キーチェーンアクセスで「**Developer ID Application: …**」を選ぶ
2. 左の三角を開き、**証明書と秘密鍵の2つを一緒に**選ぶ
3. 右クリック →「**2項目を書き出す…**」→ 形式は
   **個人情報交換(.p12)**
4. **合言葉を付ける**(後で Secrets に入れる。使い回さない)

base64 にする:

```sh
base64 -i Certificates.p12 | pbcopy   # クリップボードに入る
```

> **中身は画面に出さない。** `cat` せずに `pbcopy` で直接貼る。

---

## 3. 公証の鍵(App Store Connect API キー)を作る

Apple ID と合言葉ではなく **API キー**を使う。CI 向けにこちらが推奨で、
2要素認証に引っかからず、失効させても他に響かない。

1. <https://appstoreconnect.apple.com/access/integrations/api>
2. 「**チームキー**」の側で **＋**
3. 名前は `officework-notary` など。**役割は Developer** でよい
   (公証だけなら Admin は要らない)
4. **`.p8` は一度しか落とせない**。落として安全な所に置く
5. 同じ画面に出ている **Key ID** と、上部の **Issuer ID** を控える

base64 にする:

```sh
base64 -i AuthKey_XXXXXXXX.p8 | pbcopy
```

---

## 4. GitHub の Secrets に入れる

リポジトリの Settings → Secrets and variables → Actions → New repository secret。
**5つ**要る。

| 名前 | 中身 |
|---|---|
| `MAC_CERT_P12` | 2 で作った `.p12` の base64 |
| `MAC_CERT_PASSWORD` | その `.p12` の合言葉 |
| `MAC_API_KEY_P8` | 3 で落とした `.p8` の base64 |
| `MAC_API_KEY_ID` | 3 の Key ID(10 桁ほど) |
| `MAC_API_ISSUER_ID` | 3 の Issuer ID(UUID の形) |

名前が1つでも違うと CI がその場で止まり、**どれが無いかを名指しで言う**
(`packaging/macos/sign.sh` が最初に見る)。

---

## 5. 手元の Mac で先に試す(推奨)

CI に上げる前に、同じ台本を手元で回せる。**その機械の鍵束は汚さない**
(使い捨ての keychain を作って使う)。

```sh
export MAC_CERT_P12="$(base64 -i Certificates.p12)"
export MAC_CERT_PASSWORD='…'
export MAC_API_KEY_P8="$(base64 -i AuthKey_XXXXXXXX.p8)"
export MAC_API_KEY_ID='…'
export MAC_API_ISSUER_ID='…'

cargo build --release -p calc -p writer
# (.app を組む段は .github/workflows/release.yml の「包む」と同じ)

packaging/macos/sign.sh keychain
export SIGN_IDENTITY=…            # 上が出した SHA-1
packaging/macos/sign.sh app "dist/officework calc.app"
packaging/macos/sign.sh notarize officework-….dmg
packaging/macos/sign.sh verify   officework-….dmg
```

`verify` が出す **`spctl` の行が Gatekeeper そのもの**。ここが通れば、
利用者の機械でもそのまま開く。

---

## 何を署名しているか(読む人のために)

- **同梱 Python の Mach-O は全部が対象**。`python3`・`libpython3.x.dylib`・
  `lib-dynload/*.so`・pip で入れた拡張 — 1つでも漏れると公証が落ちる
- **`.py` は対象外**。Mach-O ではないので、利用者が置いたマクロを走らせる
  こと自体は署名の話にならない(設計の芯を曲げずに済む)
- 権利(entitlements)は [packaging/macos/entitlements.plist](../packaging/macos/entitlements.plist)
  の**2つだけ**。`pip install` した拡張を読めるようにする物と、ctypes の物。
  App Sandbox は入れない(直配布では要らず、入れると
  `~/.config/officework/` が読めなくなる)

## まだしていないこと

- **Intel(x86_64)の .dmg は出していない。** いまは arm64 だけ。
  出すなら的を増やして universal にするか、2枚出す
- **Windows の署名**(SmartScreen)は別の話。証明書の種類も出し方も違う
