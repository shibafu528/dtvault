package graph

// This file will be automatically regenerated based on the schema, any resolver implementations
// will be copied through when generating and any unknown code will be moved to the end.

import (
	"context"
	"regexp"
	"strconv"
	"time"

	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph/generated"
	"github.com/shibafu528/dtvault/graph/model"
	"github.com/vektah/gqlparser/v2/gqlerror"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func (r *programResolver) Videos(ctx context.Context, program *model.Program) ([]*model.Video, error) {
	conn, err := r.CentralAddr.Dial()
	if err != nil {
		return nil, gqlerror.Errorf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewProgramServiceClient(conn)
	res, err := client.ListVideosByProgram(ctx, &types.ListVideosByProgramRequest{
		ProgramId: &types.ProgramIdentity{
			NetworkId: uint32(program.NetworkID),
			ServiceId: uint32(program.ServiceID),
			EventId:   uint32(program.EventID),
			StartAt:   timestamppb.New(program.StartAt),
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
