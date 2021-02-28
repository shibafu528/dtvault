package main

import (
	"context"
	"fmt"
	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/playground"
	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph"
	"github.com/shibafu528/dtvault/graph/generated"
	"google.golang.org/grpc"
	"io"
	"log"
	"net/http"
	"os"
)

const defaultPort = "8080"

func main() {
	port := os.Getenv("DTVAULT_BFF_PORT")
	if port == "" {
		port = defaultPort
	}

	resolver := graph.Resolver{
		CentralAddr: "[::1]:50051",
	}
	srv := handler.NewDefaultServer(generated.NewExecutableSchema(generated.Config{Resolvers: &resolver}))

	http.Handle("/", playground.Handler("GraphQL playground", "/query"))
	http.Handle("/query", srv)
	http.HandleFunc("/stream", streamHandler)

	log.Printf("connect to http://localhost:%s/ for GraphQL playground", port)
	log.Fatal(http.ListenAndServe(":"+port, nil))
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

	conn, err := grpc.Dial("[::1]:50051", grpc.WithInsecure())
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
	var chunks [][]byte
	length := 0
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
			// skip
		case *types.GetVideoResponse_Datagram_:
			chunks = append(chunks, part.Datagram.Payload)
			length += len(part.Datagram.Payload)
		default:
			log.Printf("GetVideo: invalid response: %v", res)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			return
		}
	}

	econn, err := grpc.Dial("[::1]:50052", grpc.WithInsecure())
	if err != nil {
		log.Printf("fail to dial: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}
	defer econn.Close()

	ec := types.NewEncoderServiceClient(econn)
	pres, err := ec.ListPresets(ctx, &types.ListPresetsRequest{})
	if err != nil {
		log.Printf("ListPresets: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}

	preset := pres.Presets[0]
	vstream, err := ec.EncodeVideo(ctx)
	if err != nil {
		log.Printf("EncodeVideo: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}
	waitc := make(chan struct{})
	go func() {
		sentHeader := false
		for {
			in, err := vstream.Recv()
			if err == io.EOF {
				close(waitc)
				return
			}
			if err != nil {
				log.Printf("EncodeVideo: %v", err)
				w.WriteHeader(http.StatusInternalServerError)
				fmt.Fprint(w, "Internal error\n")
				close(waitc)
				return
			}

			switch part := in.Part.(type) {
			case *types.EncodeVideoResponse_Datagram_:
				if !sentHeader {
					w.Header().Set("Content-Type", "video/mp4")
					sentHeader = true
				}
				w.Write(part.Datagram.Payload)
			default:
				log.Printf("EncodeVideo: invalid response: %v", in)
				w.WriteHeader(http.StatusInternalServerError)
				fmt.Fprint(w, "Internal error\n")
				close(waitc)
				return
			}
		}
	}()
	sendHeader := &types.EncodeVideoRequest_Header{
		TotalLength: uint64(length),
		PresetId:    preset.PresetId,
	}
	err = vstream.Send(&types.EncodeVideoRequest{Part: &types.EncodeVideoRequest_Header_{Header: sendHeader}})
	if err != nil {
		log.Printf("EncodeVideo: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}
	sent := uint64(0)
	for _, chunk := range chunks {
		dg := &types.EncodeVideoRequest_Datagram{
			Offset:  sent,
			Payload: chunk,
		}
		err = vstream.Send(&types.EncodeVideoRequest{Part: &types.EncodeVideoRequest_Datagram_{Datagram: dg}})
		if err != nil {
			log.Printf("EncodeVideo: %v", err)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			return
		}
		sent += uint64(len(chunk))
	}
	<-waitc
}
