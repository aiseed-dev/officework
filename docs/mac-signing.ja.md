# macOS の署名と公証

配る `.dmg` に**署名して公証する**ための下ごしらえ。済ませると、利用者は
落として**そのまま開ける**(「右クリック→開く」が要らなくなる)。

出す形は **Developer ID の直配布**で、Mac App Store ではない。理由は
[docs/sekkei/ayumi.ja.md](sekkei/ayumi.ja.md) の配布の節(gpui が GPL の
3クレートを必須で引くので、Apple のストアの規約と衝突する)。

---

## 結論から — 手元の Mac なら、毎回これ1行

```sh
MAC_NOTARY_PROFILE=officework packaging/make-macos.sh
```

組んで・包んで・署名して・公証して・確かめるまで、これで終わる。

そのための下ごしらえは**2つだけ**。どちらも**一度きり**。

| | 何を | どこで |
|---|---|---|
| **A** | 証明書を用意する | Xcode で数クリック(下の A) |
| **B** | 公証の資格を鍵束に貯める | ターミナルで1行(下の B) |

> **`.p12` への書き出しは要らない。** 鍵は既にこの Mac の中にあり、
> `codesign` がそのまま使う。書き出しが要るのは、鍵の無い機械(CI)へ
> 持っていくときだけ — それは末尾の「C. CI に任せたくなったら」。

---

## A. 証明書を用意する(一度きり)

### A-1. まず、もう持っていないか聞く

ターミナル(`command + space` → `ターミナル`)で:

```sh
security find-identity -v -p codesigning
```

`Developer ID Application: 名前 (チームID)` の行が出れば**もうある** →
**A は終わり。B へ進む。**

> `Apple Development:` `Mac Developer:` `Developer ID Installer:` は**別物**。
> 前の2つは手元で動かすための物、最後は `.pkg` 用。
> **要るのは `Developer ID Application` ただ1つ。**

### A-2. Xcode で作る(いちばん短い)

Xcode があるなら、**証明書の要求(CSR)を自分で作る必要は無い**。
Xcode が裏で全部やる。

1. **Xcode** を開く
2. メニューの **Xcode → Settings…**(`command + ,`)
3. **Accounts** の柱
4. 左下の **＋** で Apple ID を足す(まだなら)。足したらチーム名を選ぶ
5. 右下の **Manage Certificates…**
6. 左下の **＋** → **Developer ID Application**
7. 一覧に出れば出来上がり。**この Mac の鍵束に入っている**

確かめる:

```sh
security find-identity -v -p codesigning
```

> **Developer ID Application が選べない**場合、そのアカウントの権限が
> 足りない(Account Holder か Admin が要る)。個人の加入なら足りている。

### A-3. Xcode を使わない道(参考)

キーチェーンアクセスで CSR を作り、Apple のサイトに出して受け取る。
Xcode が入っていない機械ではこちら。

1. キーチェーンアクセスを開く(`open -a "Keychain Access"`)
2. メニュー **キーチェーンアクセス → 証明書アシスタント →
   認証局に証明書を要求…**
3. メールと通称を入れ、**ディスクに保存**と**鍵ペア情報を指定**にチェック。
   鍵は **2048 ビット・RSA**
4. <https://developer.apple.com/account/resources/certificates/list> で
   **＋** → **Developer ID Application** → 3 のファイルを上げる
5. 出来た `.cer` を落として**ダブルクリック**(鍵束に入る)

---

## B. 公証の資格を鍵束に貯める(一度きり)

公証は Apple のサーバに出して待つ手続き。その資格を**一度だけ**貯めれば、
以後は名前で呼べる。

### B-1. App Store Connect の API キーを作る

Apple ID と合言葉より、API キーの方が手元にも CI にも向く
(2要素認証に引っかからない。失効させても他に響かない)。

1. <https://appstoreconnect.apple.com/access/integrations/api>
2. **チームキー**の側で **＋**
3. 名前は `officework-notary` など。**役割は Developer** でよい
4. **`.p8` は一度しか落とせない**。落として安全な所に置く
5. 同じ画面の **Key ID** と、上部の **Issuer ID** を控える

### B-2. 鍵束に貯める

```sh
xcrun notarytool store-credentials officework \
  --key ~/Downloads/AuthKey_XXXXXXXX.p8 \
  --key-id XXXXXXXXXX \
  --issuer xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

`officework` が**この資格の呼び名**。以後は `.p8` の場所も Key ID も
打たなくてよくなる。

---

## 毎回やること

```sh
MAC_NOTARY_PROFILE=officework packaging/make-macos.sh
```

中で起きること(全部 [packaging/make-macos.sh](../packaging/make-macos.sh) に書いてある):

1. `cargo build --release`
2. `.app` を2つ作り、**アイコンと同梱 Python(3.14)を入れる**
3. **中の Mach-O を全部署名** → `.app` を署名
4. `.app` を公証して**券を貼る**
5. `.dmg` を作って署名 → 公証 → 券を貼る
6. **`spctl` で確かめる**(= Gatekeeper そのもの。ここが通れば利用者も開ける)

出来上がりは `packaging/out/officework-<版>-macos-<的>.dmg`。
そのまま Releases に上げられる。

> 署名の下ごしらえの前に「包む所まで」を試したいなら
> `packaging/make-macos.sh --no-sign`。**配る物には付けないこと。**

---

## つまずいたら

### 「証明書が2枚あります」と止まった

更新して古い物が残っている、別チームの物が混ざっている。**どちらで署名
したか分からない物を作らない**ために、台本はここで止まる。使う方の
SHA-1(行頭の 40 桁)を控えて:

```sh
MAC_SIGN_IDENTITY=<40桁> MAC_NOTARY_PROFILE=officework packaging/make-macos.sh
```

### Apple のサイトには出るのに、`find-identity` に出ない

**証明書はあるが、対になる秘密鍵がこの Mac に無い。** 証明書だけ落として
入れても署名はできない(鍵が本体で、証明書はその身分証)。別の Mac で
作った・OS を入れ直した、のどれか。

1. **鍵を持っている Mac から `.p12` をもらう**(下の C-1 の手順で書き出した物)。
   もらったら**ダブルクリックで入れる**だけ
2. **作り直す。** 古い方は Apple のサイトで **Revoke** してよい —
   **その証明書で署名済みの物は、公証済みなら開けるまま**
   (タイムスタンプが効いているため)

> Developer ID Application はチームで **5枚まで**。使えない物が溜まって
> いたら、作り直す前に Revoke して枠を空ける。

### 証明書の期限を見たい

```sh
security find-certificate -c "Developer ID Application" -p \
  | openssl x509 -noout -subject -dates
```

`notAfter` が期限(**5年**)。切れると**それ以降に署名する物**が弾かれる。
公証済みで配り終えた物は、タイムスタンプが効いているので切れても開ける。

### 公証が通らない

台本が直近の記録を出す。多いのは「署名し漏れた Mach-O がある」で、
**同梱 Python の中**(`lib-dynload/*.so`・pip で入れた拡張)が定番。
`make-macos.sh` は中身を全部そろえてから署名するので、手で順を変えない。

---

## C. CI に任せたくなったら(いまは要らない)

GitHub Actions で自動にする道。**鍵の無い機械に鍵を渡す**ことになるので、
`.p12` への書き出しと Secrets が要る。手元で回すぶんには**全部不要**。

### C-1. 証明書を `.p12` に書き出す

```sh
security export -t identities -f pkcs12 -o Certificates.p12
```

合言葉を聞かれるので決めて打つ(これが `MAC_CERT_PASSWORD`)。
「鍵を書き出そうとしています」の窓が出たら**許可**。

> `-t identities` は「証明書と秘密鍵の対」。鍵束に対が2つ以上あると
> **全部入ってしまう**ので、その場合はキーチェーンアクセスの
> **「自分の証明書」**カテゴリから1つだけ選んで書き出す
> (「証明書」カテゴリでは `.p12` が灰色で選べない)。

確かめる(鍵そのものは出ない):

```sh
openssl pkcs12 -in Certificates.p12 -info -nokeys -noout -passin stdin
```

`Shrouded Keybag` の行があれば秘密鍵が入っている。

### C-2. Secrets に入れる

Settings → Secrets and variables → Actions。

| 名前 | 中身 |
|---|---|
| `MAC_CERT_P12` | `base64 -i Certificates.p12 \| pbcopy` の中身 |
| `MAC_CERT_PASSWORD` | C-1 で決めた合言葉 |
| `MAC_API_KEY_P8` | `base64 -i AuthKey_XXXX.p8 \| pbcopy` の中身 |
| `MAC_API_KEY_ID` | B-1 の Key ID |
| `MAC_API_ISSUER_ID` | B-1 の Issuer ID |
| `MAC_SIGN_IDENTITY` | **証明書が2枚以上あるときだけ** |

名前が1つでも違えば CI がその場で止まり、**どれが無いかを名指しで言う**。

> **`.p12` 本体は消さずに取っておく。** これが秘密鍵の控えで、Mac が
> 壊れたときに証明書を作り直さずに済む唯一の道。

---

## 何を署名しているか(読む人のために)

- **同梱 Python の Mach-O は全部が対象**。`python3`・`libpython3.x.dylib`・
  `lib-dynload/*.so`・pip で入れた拡張 — 1つでも漏れると公証が落ちる
- **`.py` は対象外**。Mach-O ではないので、利用者が置いたマクロを走らせる
  こと自体は署名の話にならない(設計の芯を曲げずに済む)
- 権利(entitlements)は
  [packaging/macos/entitlements.plist](../packaging/macos/entitlements.plist)
  の**2つだけ**。`pip install` した拡張を読めるようにする物と、ctypes の物。
  App Sandbox は入れない(直配布では要らず、入れると
  `~/.config/officework/` が読めなくなる)

## まだしていないこと

- **Intel(x86_64)の .dmg は出していない。** `make-macos.sh` は**走っている
  Mac の的**で組む(Apple Silicon なら arm64)。両方出すなら2台で回すか、
  universal に組む工事が要る
- **Windows の署名**(SmartScreen)は別の話。証明書の種類も出し方も違う
