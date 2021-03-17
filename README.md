 DTVault
====

積みTSの仕分けやdrop-checkといったファイル管理や、視聴を助けるためのシステムです。  
これ自体には録画の機能は持たず、外部システムから連携して利用する必要があります。

## Services
| Name | Description |
|----|----|
| central | メタデータの管理と動画データの保存 (将来的には、後者は別サービスに分割したい) |
| encoder-ffmpeg | ffmpegを用いてエンコードを実行 |
| bff | Webフロントエンド向けのGraphQLエンドポイント公開と、ストリーミング向けの補助処理 |
| web | Webフロントエンド |

## Commands
| Name | Description |
|----|----|
| collector-chinachu | Chinachuの録画後フックやrecorded.jsonからデータを取り込む |

## Chart

```
<Web browser>
 ↑
 ↓
[BFF]←-→[encoder]
 ↑    Encode
 ↓
[central]←-→[storage]
 ↑         Read/Write
 |
 | Send PB Normalized Program, M2TS
[collector]
 ↑
 | Send Program JSON, M2TS
<Chinachu hook or recorded.json>
```
