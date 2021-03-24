DTVault
====

積みTSの仕分けやdrop-checkといったファイル管理や、視聴を助けるためのシステム~~です~~を目指しています。  
これ自体には録画の機能は持たず、外部システムから連携して利用する必要があります。

## IMPORTANT NOTICE
現時点では、本システムは本番環境向け**ではありません**。  
データベースの形式や、映像ファイル格納先のディレクトリ構造の互換性は保証されません。

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
[central]←-→<storage>
 ↑         Read/Write
 |
 | Send PB Normalized Program, M2TS
[collector]
 ↑
 | Send Program JSON, M2TS
<Chinachu hook or recorded.json>
```

## How to build
### Rust modules
```
cargo build --release
```

モジュールごとに必要な環境変数や設定ファイルが存在するため、それぞれの main.rs や config.rs あたりを確認してください。

### Go modules
#### Generate protobuf codes
実行前に protoc や protoc-gen-go などのインストールが必要です。  
protoc-gen-go と protoc-gen-go-grpc は go get で取得できます。  
この辺は[gRPC公式サイトのガイド](https://grpc.io/docs/languages/go/quickstart/)を参考にすると良いです。

```
protoc -Iproto \
  --go_out=dtvault-types-golang \
  --go_opt=paths=source_relative \
  --go-grpc_out=dtvault-types-golang \
  --go-grpc_opt=paths=source_relative proto/**/*.proto
```

#### Generate GraphQL codes
```
go generate ./...
```

### dtvault-web
#### Generate GraphQL codes
```
yarn generate
```

開発中は `--watch` オプションを付けると自動的に再生成できます。

#### Run with dev server
```
yarn start
```

## License

Apache License 2.0 (see LICENSE file)
