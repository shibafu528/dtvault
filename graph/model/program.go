package model

import (
	"fmt"
	"time"

	types "github.com/shibafu528/dtvault/dtvault-types-golang"
)

type Program struct {
	ID          string           `json:"id"`
	NetworkID   int              `json:"networkId"`
	ServiceID   int              `json:"serviceId"`
	EventID     int              `json:"eventId"`
	StartAt     time.Time        `json:"startAt"`
	Duration    *Duration        `json:"duration"`
	Name        string           `json:"name"`
	Description string           `json:"description"`
	Extended    []*ExtendedEvent `json:"extended"`
	Service     *Service         `json:"service"`
}

func NewProgramFromPb(p *types.Program) *Program {
	ctype := ChannelType(p.Service.Channel.ChannelType.String())
	var extended []*ExtendedEvent
	for _, ext := range p.Extended {
		extended = append(extended, &ExtendedEvent{
			Key:   ext.Key,
			Value: ext.Value,
		})
	}

	return &Program{
		ID:        fmt.Sprintf("%d-%d-%d-%d", p.NetworkId, p.ServiceId, p.EventId, p.StartAt.AsTime().Unix()),
		NetworkID: int(p.NetworkId),
		ServiceID: int(p.ServiceId),
		EventID:   int(p.EventId),
		StartAt:   p.StartAt.AsTime(),
		Duration: &Duration{
			Seconds: int(p.Duration.Seconds),
			Nanos:   int(p.Duration.Nanos),
		},
		Name:        p.Name,
		Description: p.Description,
		Extended:    extended,
		Service: &Service{
			ID:        fmt.Sprintf("%d-%d", p.Service.NetworkId, p.Service.ServiceId),
			NetworkID: int(p.Service.NetworkId),
			ServiceID: int(p.Service.ServiceId),
			Name:      p.Service.Name,
			Channel: &Channel{
				ID:          fmt.Sprintf("%s-%s", ctype, p.Service.Channel.Channel),
				ChannelType: ctype,
				Channel:     p.Service.Channel.Channel,
				Name:        p.Service.Channel.Name,
			},
		},
	}
}
