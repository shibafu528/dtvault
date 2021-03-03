package graph

// This file will be automatically regenerated based on the schema, any resolver implementations
// will be copied through when generating and any unknown code will be moved to the end.

import (
	"context"
	"fmt"
	"github.com/vektah/gqlparser/v2/gqlerror"
	"google.golang.org/protobuf/types/known/timestamppb"
	"log"
	"regexp"
	"strconv"
	"time"

	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph/generated"
	"github.com/shibafu528/dtvault/graph/model"
)

func (r *queryResolver) Programs(ctx context.Context) ([]*model.Program, error) {
	conn, err := r.CentralAddr.Dial()
	if err != nil {
		log.Fatalf("fail to dial: %v", err)
	}
	defer conn.Close()

	client := types.NewProgramServiceClient(conn)
	req := types.ListProgramsRequest{}
	res, err := client.ListPrograms(ctx, &req)
	if err != nil {
		log.Fatalf("ListPrograms: %v", err)
	}

	var programs []*model.Program
	for _, program := range res.Programs {
		res2, err := client.ListVideosByProgram(ctx, &types.ListVideosByProgramRequest{
			ProgramId: &types.ProgramIdentity{
				NetworkId: program.NetworkId,
				ServiceId: program.ServiceId,
				EventId:   program.EventId,
				StartAt:   program.StartAt,
			},
		})
		if err != nil {
			log.Fatalf("ListVideosByProgram: %v", err)
		}

		ctype := model.ChannelType(program.Service.Channel.ChannelType.String())
		var extended []*model.ExtendedEvent
		for _, ext := range program.Extended {
			extended = append(extended, &model.ExtendedEvent{
				Key:   ext.Key,
				Value: ext.Value,
			})
		}
		var videos []*model.Video
		for _, video := range res2.Videos {
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

		programs = append(programs, &model.Program{
			ID:        fmt.Sprintf("%d-%d-%d-%d", program.NetworkId, program.ServiceId, program.EventId, program.StartAt.AsTime().Unix()),
			NetworkID: int(program.NetworkId),
			ServiceID: int(program.ServiceId),
			EventID:   int(program.EventId),
			StartAt:   program.StartAt.AsTime(),
			Duration: &model.Duration{
				Seconds: int(program.Duration.Seconds),
				Nanos:   int(program.Duration.Nanos),
			},
			Name:        program.Name,
			Description: program.Description,
			Extended:    extended,
			Service: &model.Service{
				ID:        fmt.Sprintf("%d-%d", program.Service.NetworkId, program.Service.ServiceId),
				NetworkID: int(program.Service.NetworkId),
				ServiceID: int(program.Service.ServiceId),
				Name:      program.Service.Name,
				Channel: &model.Channel{
					ID:          fmt.Sprintf("%s-%s", ctype, program.Service.Channel.Channel),
					ChannelType: ctype,
					Channel:     program.Service.Channel.Channel,
					Name:        program.Service.Channel.Name,
				},
			},
			Videos: videos,
		})
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
		log.Fatalf("fail to dial: %v", err)
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
		log.Fatalf("GetProgram: %v", err)
	}

	res2, err := client.ListVideosByProgram(ctx, &types.ListVideosByProgramRequest{
		ProgramId: &types.ProgramIdentity{
			NetworkId: res.Program.NetworkId,
			ServiceId: res.Program.ServiceId,
			EventId:   res.Program.EventId,
			StartAt:   res.Program.StartAt,
		},
	})
	if err != nil {
		log.Fatalf("ListVideosByProgram: %v", err)
	}

	ctype := model.ChannelType(res.Program.Service.Channel.ChannelType.String())
	var extended []*model.ExtendedEvent
	for _, ext := range res.Program.Extended {
		extended = append(extended, &model.ExtendedEvent{
			Key:   ext.Key,
			Value: ext.Value,
		})
	}
	var videos []*model.Video
	for _, video := range res2.Videos {
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

	program := &model.Program{
		ID:        fmt.Sprintf("%d-%d-%d-%d", res.Program.NetworkId, res.Program.ServiceId, res.Program.EventId, res.Program.StartAt.AsTime().Unix()),
		NetworkID: int(res.Program.NetworkId),
		ServiceID: int(res.Program.ServiceId),
		EventID:   int(res.Program.EventId),
		StartAt:   res.Program.StartAt.AsTime(),
		Duration: &model.Duration{
			Seconds: int(res.Program.Duration.Seconds),
			Nanos:   int(res.Program.Duration.Nanos),
		},
		Name:        res.Program.Name,
		Description: res.Program.Description,
		Extended:    extended,
		Service: &model.Service{
			ID:        fmt.Sprintf("%d-%d", res.Program.Service.NetworkId, res.Program.Service.ServiceId),
			NetworkID: int(res.Program.Service.NetworkId),
			ServiceID: int(res.Program.Service.ServiceId),
			Name:      res.Program.Service.Name,
			Channel: &model.Channel{
				ID:          fmt.Sprintf("%s-%s", ctype, res.Program.Service.Channel.Channel),
				ChannelType: ctype,
				Channel:     res.Program.Service.Channel.Channel,
				Name:        res.Program.Service.Channel.Name,
			},
		},
		Videos: videos,
	}

	return program, nil
}

// Query returns generated.QueryResolver implementation.
func (r *Resolver) Query() generated.QueryResolver { return &queryResolver{r} }

type queryResolver struct{ *Resolver }
