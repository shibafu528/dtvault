package emitter

import (
	"context"
	"errors"
	"fmt"
	"github.com/shibafu528/dtvault/dtvault-bff/grpcaddr"
	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/dtvault-types-golang/encoder"
	"golang.org/x/xerrors"
	"google.golang.org/grpc"
	"io"
	"log"
	"net/http"
)

type Encoder struct {
	conn   *grpc.ClientConn
	video  *types.Video
	preset *encoder.Preset
}

var ErrPresetNotFound = errors.New("preset not found")

func NewEncoder(encoderAddr *grpcaddr.Address, presetId string) (*Encoder, error) {
	conn, err := encoderAddr.Dial()
	if err != nil {
		return nil, xerrors.Errorf("fail to dial: %+w", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	client := encoder.NewEncoderServiceClient(conn)
	res, err := client.ListPresets(ctx, &encoder.ListPresetsRequest{})
	if err != nil {
		return nil, xerrors.Errorf("error in call EncoderService.ListPresets(): %+w", err)
	}

	var preset *encoder.Preset
	for _, p := range res.Presets {
		if p.PresetId == presetId {
			preset = p
			break
		}
	}
	if preset == nil {
		return nil, xerrors.Errorf("%+w", ErrPresetNotFound)
	}

	return &Encoder{conn: conn, preset: preset}, nil
}

func (e *Encoder) SetVideo(v *types.Video) {
	e.video = v
}

func (e *Encoder) Run(in <-chan []byte, w http.ResponseWriter) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	client := encoder.NewEncoderServiceClient(e.conn)
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
			case *encoder.EncodeVideoResponse_Datagram_:
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
			dg := &encoder.EncodeVideoRequest_Header{
				TotalLength: e.video.TotalLength,
				PresetId:    e.preset.PresetId,
			}
			err = stream.Send(&encoder.EncodeVideoRequest{Part: &encoder.EncodeVideoRequest_Header_{Header: dg}})
			if err != nil {
				log.Printf("EncodeVideo send header error: %v", err)
				w.WriteHeader(http.StatusInternalServerError)
				fmt.Fprint(w, "Internal error\n")
				return
			}

			init = true
		}

		dg := &encoder.EncodeVideoRequest_Datagram{
			Offset:  sent,
			Payload: i,
		}
		err = stream.Send(&encoder.EncodeVideoRequest{Part: &encoder.EncodeVideoRequest_Datagram_{Datagram: dg}})
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

func (e *Encoder) Close() error {
	return e.conn.Close()
}
