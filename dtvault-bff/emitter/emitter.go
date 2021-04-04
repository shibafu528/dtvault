package emitter

import (
	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"net/http"
)

type Emitter interface {
	SetVideo(v *types.Video)
	Run(in <-chan []byte, w http.ResponseWriter)
	Close() error
}
