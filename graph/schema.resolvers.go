package graph

// This file will be automatically regenerated based on the schema, any resolver implementations
// will be copied through when generating and any unknown code will be moved to the end.

import (
	"context"
	"encoding/base64"
	"fmt"
	"io"
	"log"
	"regexp"
	"strconv"
	"strings"
	"time"

	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph/generated"
	"github.com/shibafu528/dtvault/graph/model"
	"github.com/vektah/gqlparser/v2/gqlerror"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func (r *programResolver) Videos(ctx context.Context, obj *model.Program) ([]*model.Video, error) {
	conn, err := r.CentralAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewProgramServiceClient(conn)
	res, err := client.ListVideosByProgram(ctx, &types.ListVideosByProgramRequest{
		ProgramId: &types.ProgramIdentity{
			NetworkId: uint32(obj.NetworkID),
			ServiceId: uint32(obj.ServiceID),
			EventId:   uint32(obj.EventID),
			StartAt:   timestamppb.New(obj.StartAt),
		},
	})
	if err != nil {
		return nil, gqlerror.Errorf("ListVideosByProgram: %v", err)
	}

	var videos []*model.Video
	for _, video := range res.Videos {
		videos = append(videos, &model.Video{
			ID:          video.VideoId,
			ProviderID:  video.ProviderId,
			TotalLength: strconv.FormatUint(video.TotalLength, 10),
			FileName:    video.FileName,
			MimeType:    video.MimeType,
			StorageID:   video.StorageId,
			Prefix:      video.Prefix,
		})
	}

	return videos, nil
}

func (r *programResolver) Thumbnail(ctx context.Context, obj *model.Program) (*string, error) {
	conn, err := r.CentralAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewProgramServiceClient(conn)
	res, err := client.ListVideosByProgram(ctx, &types.ListVideosByProgramRequest{
		ProgramId: &types.ProgramIdentity{
			NetworkId: uint32(obj.NetworkID),
			ServiceId: uint32(obj.ServiceID),
			EventId:   uint32(obj.EventID),
			StartAt:   timestamppb.New(obj.StartAt),
		},
	})
	if err != nil {
		return nil, gqlerror.Errorf("ListVideosByProgram: %v", err)
	}
	if len(res.Videos) == 0 {
		return nil, nil
	}

	econn, err := r.EncoderAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer econn.Close()

	enc := types.NewEncoderServiceClient(econn)
	stream, err := enc.GenerateThumbnail(ctx)
	if err != nil {
		return nil, gqlerror.Errorf("GenerateThumbnail: %v", err)
	}

	term := make(chan struct{})
	done := make(chan struct{})
	verr := make(chan error)
	go func() {
		defer close(verr)
		defer close(done)

		vss := types.NewVideoStorageServiceClient(conn)
		req := &types.GetVideoRequest{VideoId: res.Videos[0].VideoId}
		vstream, err := vss.GetVideo(ctx, req)
		if err != nil {
			verr <- err
			return
		}

		for {
			select {
			case <-term:
				err := vstream.CloseSend()
				if err != nil {
					verr <- err
				}
				return
			default:
			}

			r, err := vstream.Recv()
			if err == io.EOF {
				break
			}

			switch part := r.Part.(type) {
			case *types.GetVideoResponse_Header:
				req := &types.GenerateThumbnailRequest_Header{
					TotalLength:  part.Header.TotalLength,
					OutputFormat: types.GenerateThumbnailRequest_OUTPUT_FORMAT_JPEG,
					Width:        1280,
					Height:       720,
					Position:     30,
				}
				err = stream.Send(&types.GenerateThumbnailRequest{Part: &types.GenerateThumbnailRequest_Header_{Header: req}})
				if err != nil {
					verr <- err
					return
				}
			case *types.GetVideoResponse_Datagram_:
				req := &types.GenerateThumbnailRequest_Datagram{
					Offset:  part.Datagram.Offset,
					Payload: part.Datagram.Payload,
				}
				err = stream.Send(&types.GenerateThumbnailRequest{Part: &types.GenerateThumbnailRequest_Datagram_{Datagram: req}})
				if err != nil {
					verr <- err
					return
				}
			default:
				log.Printf("EncodeVideo: invalid response: %v", req)
			}
		}
	}()

	var blob []byte
	for {
		select {
		case err := <-verr:
			err2 := stream.CloseSend()
			if err2 != nil {
				log.Printf("GenerateThumbnail: %v", err2)
			}
			return nil, gqlerror.Errorf("GetVideo: %v", err)
		default:
		}

		r, err := stream.Recv()
		if err == io.EOF || (err != nil && strings.Contains(err.Error(), "Broken pipe")) {
			// TODO: サーバ側の通信の切り方が間違っているようなので、一旦EOF以外でも終了扱いにしている。実際は err == io.EOF のみが正しい。
			break
		}
		if err != nil {
			close(term)
			return nil, gqlerror.Errorf("GenerateThumbnail: %v", err)
		}

		switch part := r.Part.(type) {
		case *types.GenerateThumbnailResponse_Datagram_:
			blob = append(blob, part.Datagram.Payload...)
		default:
			log.Printf("GenerateThumbnail: invalid response: %v", r)
		}
	}
	close(term)

	err = <-verr
	if err != nil {
		log.Printf("GetVideo: %v", err)
	}

	<-done

	if len(blob) == 0 {
		return nil, nil
	}

	uri := fmt.Sprintf("data:image/jpeg;base64,%s", base64.StdEncoding.EncodeToString(blob))
	return &uri, nil
}

func (r *queryResolver) Programs(ctx context.Context) ([]*model.Program, error) {
	conn, err := r.CentralAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewProgramServiceClient(conn)
	req := types.ListProgramsRequest{}
	res, err := client.ListPrograms(ctx, &req)
	if err != nil {
		return nil, gqlerror.Errorf("ListPrograms: %v", err)
	}

	var programs []*model.Program
	for _, program := range res.Programs {
		programs = append(programs, model.NewProgramFromPb(program))
	}

	return programs, nil
}

func (r *queryResolver) Program(ctx context.Context, id string) (*model.Program, error) {
	re := regexp.MustCompile(`\A(\d+)-(\d+)-(\d+)-(\d+)\z`)
	matches := re.FindStringSubmatch(id)
	if matches == nil {
		return nil, gqlerror.Errorf("Invalid id")
	}
	nid, _ := strconv.ParseUint(matches[1], 10, 0)
	sid, _ := strconv.ParseUint(matches[2], 10, 0)
	eid, _ := strconv.ParseUint(matches[3], 10, 0)
	at, _ := strconv.ParseInt(matches[4], 10, 0)

	conn, err := r.CentralAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewProgramServiceClient(conn)
	req := types.GetProgramRequest{ProgramId: &types.ProgramIdentity{
		NetworkId: uint32(nid),
		ServiceId: uint32(sid),
		EventId:   uint32(eid),
		StartAt:   timestamppb.New(time.Unix(at, 0)),
	}}
	res, err := client.GetProgram(ctx, &req)
	if err != nil {
		return nil, gqlerror.Errorf("GetProgram: %v", err)
	}

	return model.NewProgramFromPb(res.Program), nil
}

func (r *queryResolver) Presets(ctx context.Context) ([]*model.Preset, error) {
	conn, err := r.EncoderAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewEncoderServiceClient(conn)
	req := types.ListPresetsRequest{}
	res, err := client.ListPresets(ctx, &req)
	if err != nil {
		return nil, gqlerror.Errorf("ListPresets: %v", err)
	}

	var presets []*model.Preset
	for _, preset := range res.Presets {
		presets = append(presets, &model.Preset{
			ID:      preset.PresetId,
			Title:   &preset.Title,
			Command: preset.Command,
		})
	}

	return presets, nil
}

// Program returns generated.ProgramResolver implementation.
func (r *Resolver) Program() generated.ProgramResolver { return &programResolver{r} }

// Query returns generated.QueryResolver implementation.
func (r *Resolver) Query() generated.QueryResolver { return &queryResolver{r} }

type programResolver struct{ *Resolver }
type queryResolver struct{ *Resolver }
