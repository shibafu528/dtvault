package main

import (
	"context"
	"fmt"
	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/playground"
	"github.com/shibafu528/dtvault/dtvault-bff/grpcaddr"
	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph"
	"github.com/shibafu528/dtvault/graph/generated"
	"golang.org/x/xerrors"
	"io"
	"log"
	"net/http"
	"os"
)

const defaultPort = "8080"

var centralAddr *grpcaddr.Address
var encoderAddr *grpcaddr.Address

func main() {
	var err error
	port := os.Getenv("DTVAULT_BFF_PORT")
	if port == "" {
		port = defaultPort
	}
	centralAddr, err = addrFromEnv("DTVAULT_CENTRAL_ADDR")
	if err != nil {
		log.Fatal(err)
	}
	encoderAddr, err = addrFromEnv("DTVAULT_ENCODER_ADDR")
	if err != nil {
		log.Fatal(err)
	}

	resolver := graph.Resolver{
		CentralAddr: centralAddr,
		EncoderAddr: encoderAddr,
	}
	srv := handler.NewDefaultServer(generated.NewExecutableSchema(generated.Config{Resolvers: &resolver}))

	http.Handle("/", playground.Handler("GraphQL playground", "/query"))
	http.Handle("/query", srv)
	http.HandleFunc("/stream", streamHandler)

	log.Printf("connect to http://localhost:%s/ for GraphQL playground", port)
	log.Fatal(http.ListenAndServe(":"+port, nil))
}

func addrFromEnv(key string) (*grpcaddr.Address, error) {
	url := os.Getenv(key)
	if url == "" {
		return nil, xerrors.Errorf("missing environment variable `%s`", key)
	}

	addr, err := grpcaddr.Parse(url)
	if err != nil {
		return nil, xerrors.Errorf("Invalid environment variable `%s`: %+w", key, err)
	}

	return addr, nil
}

func streamHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		w.WriteHeader(http.StatusNotFound)
		fmt.Fprint(w, "Not found\n")
		return
	}

	q := r.URL.Query()
	id := q.Get("id")
	if id == "" {
		w.WriteHeader(http.StatusBadRequest)
		fmt.Fprint(w, "Missing id\n")
		return
	}
	//p := q.Get("preset")

	conn, err := centralAddr.Dial()
	if err != nil {
		log.Printf("fail to dial: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}
	defer conn.Close()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	vssc := types.NewVideoStorageServiceClient(conn)
	stream, err := vssc.GetVideo(ctx, &types.GetVideoRequest{VideoId: id})
	if err != nil {
		log.Printf("GetVideo: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}
	for {
		res, err := stream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			log.Printf("GetVideo: %v", err)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			return
		}
		switch part := res.Part.(type) {
		case *types.GetVideoResponse_Header:
			w.Header().Set("Content-Type", part.Header.MimeType)
			// パススルー出力の場合はファイル名とか出せる
			w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%s", part.Header.FileName))
			//w.Header().Set("Content-Length", strconv.FormatUint(part.Header.TotalLength, 10))
		case *types.GetVideoResponse_Datagram_:
			w.Write(part.Datagram.Payload)
		default:
			log.Printf("GetVideo: invalid response: %v", res)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			return
		}
	}
}
