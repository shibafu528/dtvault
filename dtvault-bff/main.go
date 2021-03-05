package main

import (
	"context"
	"errors"
	"fmt"
	"github.com/99designs/gqlgen/graphql/handler"
	"github.com/99designs/gqlgen/graphql/playground"
	"github.com/shibafu528/dtvault/dtvault-bff/grpcaddr"
	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph"
	"github.com/shibafu528/dtvault/graph/generated"
	"golang.org/x/xerrors"
	"google.golang.org/grpc"
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

	var e emitter
	p := q.Get("preset")
	if p == "" {
		e = &passthru{}
	} else {
		var err error
		e, err = newEncoder(p)
		if err != nil {
			log.Printf("error in initializing encoder: %v", err)
			if errors.Is(err, ErrPresetNotFound) {
				w.WriteHeader(http.StatusBadRequest)
				fmt.Fprintf(w, "Preset '%s' not found\n", p)
			} else {
				w.WriteHeader(http.StatusInternalServerError)
				fmt.Fprint(w, "Internal error\n")
			}
			return
		}
	}
	defer e.close()

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

	emit := make(chan []byte)
	go e.run(emit, w)
	for {
		res, err := stream.Recv()
		if err == io.EOF {
			break
		}
		if err != nil {
			log.Printf("GetVideo: %v", err)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			break
		}
		switch part := res.Part.(type) {
		case *types.GetVideoResponse_Header:
			e.setVideo(part.Header)
		case *types.GetVideoResponse_Datagram_:
			emit <- part.Datagram.Payload
		default:
			log.Printf("GetVideo: invalid response: %v", res)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			break
		}
	}
	close(emit)
}

type emitter interface {
	setVideo(v *types.Video)
	run(in <-chan []byte, w http.ResponseWriter)
	close() error
}

type passthru struct {
	video *types.Video
}

func (p *passthru) setVideo(v *types.Video) {
	p.video = v
}

func (p *passthru) run(in <-chan []byte, w http.ResponseWriter) {
	init := false
	for i := range in {
		if !init {
			w.Header().Set("Content-Type", p.video.MimeType)
			// パススルー出力の場合はファイル名とか出せる
			w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%s", p.video.FileName))
			//w.Header().Set("Content-Length", strconv.FormatUint(part.Header.TotalLength, 10))
			init = true
		}
		w.Write(i)
	}
}

func (p *passthru) close() error {
	return nil
}

type encoder struct {
	conn   *grpc.ClientConn
	video  *types.Video
	preset *types.Preset
}

var ErrPresetNotFound = errors.New("preset not found")

func newEncoder(presetId string) (*encoder, error) {
	conn, err := encoderAddr.Dial()
	if err != nil {
		return nil, xerrors.Errorf("fail to dial: %+w", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	client := types.NewEncoderServiceClient(conn)
	res, err := client.ListPresets(ctx, &types.ListPresetsRequest{})
	if err != nil {
		return nil, xerrors.Errorf("error in call EncoderService.ListPresets(): %+w", err)
	}

	var preset *types.Preset
	for _, p := range res.Presets {
		if p.PresetId == presetId {
			preset = p
			break
		}
	}
	if preset == nil {
		return nil, xerrors.Errorf("%+w", ErrPresetNotFound)
	}

	return &encoder{conn: conn, preset: preset}, nil
}

func (e *encoder) setVideo(v *types.Video) {
	e.video = v
}

func (e *encoder) run(in <-chan []byte, w http.ResponseWriter) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	client := types.NewEncoderServiceClient(e.conn)
	stream, err := client.EncodeVideo(ctx)
	if err != nil {
		log.Printf("EncodeVideo connection error: %v", err)
		w.WriteHeader(http.StatusInternalServerError)
		fmt.Fprint(w, "Internal error\n")
		return
	}

	done := make(chan interface{})
	go func() {
		init := false
		for {
			out, err := stream.Recv()
			if err == io.EOF {
				break
			}
			if err != nil {
				log.Printf("EncodeVideo receive error: %v", err)
				w.WriteHeader(http.StatusInternalServerError)
				fmt.Fprint(w, "Internal error\n")
				break
			}

			switch part := out.Part.(type) {
			case *types.EncodeVideoResponse_Datagram_:
				if !init {
					w.Header().Set("Content-Type", "video/mp4")
					init = true
				}
				w.Write(part.Datagram.Payload)
			default:
				log.Printf("EncodeVideo: invalid response: %v", in)
			}
		}
		close(done)
	}()

	init := false
	sent := uint64(0)
	for i := range in {
		if !init {
			dg := &types.EncodeVideoRequest_Header{
				TotalLength: e.video.TotalLength,
				PresetId:    e.preset.PresetId,
			}
			err = stream.Send(&types.EncodeVideoRequest{Part: &types.EncodeVideoRequest_Header_{Header: dg}})
			if err != nil {
				log.Printf("EncodeVideo send header error: %v", err)
				w.WriteHeader(http.StatusInternalServerError)
				fmt.Fprint(w, "Internal error\n")
				return
			}

			init = true
		}

		dg := &types.EncodeVideoRequest_Datagram{
			Offset:  sent,
			Payload: i,
		}
		err = stream.Send(&types.EncodeVideoRequest{Part: &types.EncodeVideoRequest_Datagram_{Datagram: dg}})
		if err != nil {
			log.Printf("EncodeVideo send datagram error: %v", err)
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Fprint(w, "Internal error\n")
			return
		}
		sent += uint64(len(i))
	}
	<-done
}

func (e *encoder) close() error {
	return e.conn.Close()
}
