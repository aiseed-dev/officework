# macOS の署名と公証

配る `.dmg` に**署名して公証する**ための下ごしらえ。済ませると、利用者は
落として**そのまま開ける**(「右クリック→開く」が要らなくなる)。

出す形は **Developer ID の直配布**で、Mac App Store ではない。理由は
[docs/sekkei/ayumi.ja.md](sekkei/ayumi.ja.md) の配布の節(gpui が GPL の
3クレートを必須で引くので、Apple のストアの規約と衝突する)。

---

## 結論から — 下ごしらえは2つ、以後はタグを押すだけ

**GitHub に作らせる。** タグを押せば署名・公証つきの `.dmg` が Releases に
出る。手元でやることは**一度きりの下ごしらえ**だけ。

| | 何を | どこで |
|---|---|---|
| **A** | 証明書を用意する | Xcode で数クリック(下の A) |
| **B** | 公証の API キーを作る | Apple のサイトで(下の B) |
| **C** | 秘密を GitHub に入れる | **Mac でこの1行**(下の C) |

C はこれだけ:

```sh
packaging/macos/setup-ci-secrets.sh ~/Downloads/AuthKey_XXXX.p8 <Issuer ID>
```

`.p12` の書き出しも base64 も貼り付けも、この台本が繋いでやる。
**秘密は画面に出ない**(`gh` へ直に流す)。

以後は毎回:

```sh
git tag app-v0.1.0-alpha && git push origin app-v0.1.0-alpha
```

> **手元の Mac で作ることもできる**(下の「D. 手元で作る」)。
> そちらは `.p12` も GitHub も要らないので、**下ごしらえの前に一度試して
> みる**のに向く。出来上がる物は同じ。

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

## B. 公証の API キーを作る(一度きり)

公証は Apple のサーバに出して待つ手続き。その資格が要る。
Apple ID と合言葉より、**API キー**の方が向く(2要素認証に引っかからない。
失効させても他に響かない)。

**要るのは3つ**: `.p8` のファイル・**Key ID**・**Issuer ID**。
どれも同じ画面で手に入る。

### B-1. 作る

**入る場所はここ**(直に開ける):

<https://appstoreconnect.apple.com/access/integrations/api>

画面から辿るなら **App Store Connect** → 上の **ユーザとアクセス**
(Users and Access)→ **キー**(Integrations / Keys)→
**App Store Connect API** → **チームキー**(Team Keys)。

1. **＋** を押す
2. 名前は `officework-notary` など(後から見て分かればよい)
3. **アクセス(役割)は「Developer」でよい** — 公証だけなら Admin は要らない
4. **生成** を押す

> **作れない・＋ が無い**場合、そのアカウントの権限が足りない
> (Account Holder か Admin が要る)。

### B-2. 3つを受け取る

作ると一覧に1行増える。**そこに全部ある。**

| 要る物 | どこ | 形 |
|---|---|---|
| `.p8` のファイル | その行の **「ダウンロード」** | `AuthKey_XXXXXXXXXX.p8` |
| **Key ID** | その行の **「キー ID」**の列 | 10 桁の英数字 |
| **Issuer ID** | **表の上**に出ている「Issuer ID」(横に「コピー」) | UUID(`8a...-....-....-....-............`) |

> **`.p8` は一度しか落とせない。** 落とし損ねたら、その鍵は捨てて
> 作り直すしかない(失効させて＋からやり直す)。安全な所に置く。

### B-3. Key ID は**ファイル名にも入っている**

落とした `.p8` の名前が答え:

```
AuthKey_ABCD123456.p8
        ^^^^^^^^^^  ← これが Key ID
```

だから、あとで Key ID を控え損ねても `.p8` さえあれば分かる。
C の台本も**ファイル名から読み取る**ので、打たなくてよい。

> **Issuer ID だけはファイルから分からない。** 上の画面でコピーして
> 控えておくこと(チームで1つなので、一度控えれば使い回せる)。

---

## C. 秘密を GitHub に入れる(一度きり)

```sh
packaging/macos/setup-ci-secrets.sh ~/Downloads/AuthKey_XXXX.p8 <Issuer ID>
```

この台本がやること:

1. 証明書があるか確かめる(2枚以上なら**どれを使うか聞く**)
2. `.p12` に書き出す(**合言葉を2回聞かれる** — 自分で決める物)
3. base64 にして `gh` で GitHub の Secrets へ入れる。
   **値は画面にもクリップボードにも出ない**
4. 入った名前だけを一覧で見せる

要る物:

- **GitHub CLI** — `brew install gh && gh auth login`
- A の証明書と、B の `.p8` と **Issuer ID**
  (**Key ID は要らない** — `.p8` のファイル名から読む。
  名前を変えてしまったなら3つ目に書く)

> **書き出した `.p12` は消さないこと**(Desktop に置かれる)。
> この Mac が壊れたとき、証明書を作り直さずに済む唯一の控え。
> 安全な所へ移して、合言葉はパスワード管理に残す。

> 他人と共有している Mac では、この台本は使わず末尾の「手で入れる」へ。
> 合言葉が一瞬 `ps` に見えるため。

入った物は**名前だけ**ここで見られる(中身は二度と見られない):

<https://github.com/aiseed-dev/officework/settings/secrets/actions>

### 入る秘密(名前だけ)

| 名前 | 中身 |
|---|---|
| `MAC_CERT_P12` | 証明書と秘密鍵(`.p12` の base64) |
| `MAC_CERT_PASSWORD` | その合言葉 |
| `MAC_API_KEY_P8` | 公証の API キー(`.p8` の base64) |
| `MAC_API_KEY_ID` | Key ID |
| `MAC_API_ISSUER_ID` | Issuer ID |
| `MAC_SIGN_IDENTITY` | **証明書が2枚以上あるときだけ** |

---

## 毎回やること — タグを押す

```sh
git tag app-v0.1.0-alpha
git push origin app-v0.1.0-alpha
```

GitHub の macOS の機械が、手元と**同じ台本**
([packaging/make-macos.sh](../packaging/make-macos.sh))を回す:

1. `cargo build --release`
2. `.app` を2つ作り、**アイコンと同梱 Python(3.14)を入れる**
3. **中の Mach-O を全部署名** → `.app` を署名
4. `.app` を公証して**券を貼る**
5. `.dmg` を作って署名 → 公証 → 券を貼る
6. **`spctl` で確かめる**(= Gatekeeper そのもの。ここが通れば利用者も開ける)

秘密が入っていなければ**署名せずに包み**、名前に `-unsigned` が付く。
落ちはしないので、取り違えて配ることだけ気をつければよい。

---

## D. 手元の Mac で作る(下ごしらえの前に試すなら)

`.p12` も GitHub も要らない。鍵は既にこの Mac の鍵束に居て、`codesign` が
そのまま使う。

公証の資格だけ一度貯める:

```sh
xcrun notarytool store-credentials officework \
  --key ~/Downloads/AuthKey_XXXXXXXX.p8 \
  --key-id XXXXXXXXXX \
  --issuer xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

以後:

```sh
MAC_NOTARY_PROFILE=officework packaging/make-macos.sh
```

出来上がりは `packaging/out/officework-<版>-macos-<的>.dmg`。

> 署名の下ごしらえの前に「包む所まで」だけ試すなら
> `packaging/make-macos.sh --no-sign`。**配る物には付けないこと**
> (名前に `-unsigned` が付く)。

## つまずいたら

### 「証明書が2枚あります」と止まった

更新して古い物が残っている、別チームの物が混ざっている。**どちらで署名
したか分からない物を作らない**ために、台本はここで止まる。使う方の
SHA-1(行頭の 40 桁)を控えて:

GitHub に作らせているなら Secrets に `MAC_SIGN_IDENTITY` を足す
(C の台本は2枚以上あると聞いてくるので、自分で足す必要は無い)。
手元で回すなら:

```sh
MAC_SIGN_IDENTITY=<40桁> MAC_NOTARY_PROFILE=officework packaging/make-macos.sh
```

### Apple のサイトには出るのに、`find-identity` に出ない

**証明書はあるが、対になる秘密鍵がこの Mac に無い。** 証明書だけ落として
入れても署名はできない(鍵が本体で、証明書はその身分証)。別の Mac で
作った・OS を入れ直した、のどれか。

1. **鍵を持っている Mac から `.p12` をもらう**(末尾「手で入れる」の1の手順で書き出した物)。
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

## 手で入れる(`gh` を使わない場合)

C の台本が使えない・使いたくないときの道。やることは同じ。

### 1. 証明書を `.p12` に書き出す

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

### 2. Secrets に貼る

**入れる場所はここ**(直に開ける):

<https://github.com/aiseed-dev/officework/settings/secrets/actions>

画面から辿るなら:

1. リポジトリの上のタブで **Settings**
   (**アカウント**の Settings ではない。リポジトリの名前の下に並ぶ方)
2. 左の柱の **Secrets and variables** → **Actions**
   (Codespaces / Dependabot もあるが、**Actions** を選ぶ)
3. **Repository secrets** の側の緑の **New repository secret**
   (**Environment secrets** ではない)
4. **Name** に `MAC_CERT_P12`、**Secret** に中身を貼って **Add secret**
5. 残りの4つ(または5つ)も同じように足す

> **Settings のタブが見えない**なら、そのリポジトリへの権限が足りない
> (管理者が要る)。
>
> **一度入れた中身は二度と見られない。** 名前の一覧と「いつ更新したか」
> だけが出る。間違えたら **Update** で入れ直す(消して作り直さなくてよい)。

名前は C の表のとおり。base64 はクリップボード経由で:

```sh
base64 -i Certificates.p12 | pbcopy          # MAC_CERT_P12
base64 -i AuthKey_XXXX.p8  | pbcopy          # MAC_API_KEY_P8
```

> **中身は画面に出さない。** `cat` せずに `pbcopy` で直接貼る。
> 貼り終えたら `pbcopy < /dev/null` でクリップボードを空にしておく。

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
