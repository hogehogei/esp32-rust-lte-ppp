# esp32-rust-lte-ppp
esp32 &amp; Rust std Point to Point Protocol using lte module.

# 環境構築
WSL上に環境構築。必要であればLTSのバージョン更新も実施。
```
$ sudo apt update
$ sudo apt upgrade
$ sudo apt dist-upgrade && sudo apt install update-manager-core
$ sudo do-release-upgrade -d
```
cargo で必要なパッケージをインストールして、テンプレートプロジェクトを作成。
```
$ cargo install ldproxy
$ cargo install espup
$ espup install
$ cargo install cargo-generate
$ cargo generate --vcs none --git https://github.com/esp-rs/esp-idf-template cargo
```
# FW書き込み
フラッシュへ書き込み用ツールダウンロード
```
$ cargo install cargo-espflash
```
フラッシュへ書き込みするために、Windows側でUSBをWSLにアタッチ。<br>
usbipd をインストールしておく。
```
> usbipd list
> usbipd attach --busid xxxx
(実行時にbindしろと怒られたらその通りに実施してから再度attach)
```
WSL側でデバイスが見えているか確認して、アクセス権限を付与しておく。
cargo run コマンドでビルドとファーム書き込みを実施できる。
```
$ dmesg
$ lsusb
$ chmod 777 /dev/ttyUSB0
$ cargo run
```
