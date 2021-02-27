package graph

// This file will be automatically regenerated based on the schema, any resolver implementations
// will be copied through when generating and any unknown code will be moved to the end.

import (
	"context"
	"fmt"
	"log"

	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"github.com/shibafu528/dtvault/graph/generated"
	"github.com/shibafu528/dtvault/graph/model"
	"google.golang.org/grpc"
)

func (r *queryResolver) Programs(ctx context.Context) ([]*model.Program, error) {
	conn, err := grpc.Dial("[::1]:50051", grpc.WithInsecure())
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
		ctype := model.ChannelType(program.Service.Channel.ChannelType.String())
		var extended []*model.ExtendedEvent
		for _, ext := range program.Extended {
			extended = append(extended, &model.ExtendedEvent{
				Key:   ext.Key,
				Value: ext.Value,
			})
		}

		programs = append(programs, &model.Program{
			ID:        fmt.Sprintf("%d:%d:%d:%d", program.NetworkId, program.ServiceId, program.EventId, program.StartAt.AsTime().Unix()),
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
				ID:        fmt.Sprintf("%d:%d", program.Service.NetworkId, program.Service.ServiceId),
				NetworkID: int(program.Service.NetworkId),
				ServiceID: int(program.Service.ServiceId),
				Name:      program.Service.Name,
				Channel: &model.Channel{
					ID:          fmt.Sprintf("%s:%s", ctype, program.Service.Channel.Channel),
					ChannelType: ctype,
					Channel:     program.Service.Channel.Channel,
					Name:        program.Service.Channel.Name,
				},
			},
			Videos: nil,
		})
	}

	return programs, nil
}

// Query returns generated.QueryResolver implementation.
func (r *Resolver) Query() generated.QueryResolver { return &queryResolver{r} }

type queryResolver struct{ *Resolver }
