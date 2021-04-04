package emitter

import (
	"fmt"
	types "github.com/shibafu528/dtvault/dtvault-types-golang"
	"net/http"
)

type Passthru struct {
	video *types.Video
}

func (p *Passthru) SetVideo(v *types.Video) {
	p.video = v
}

func (p *Passthru) Run(in <-chan []byte, w http.ResponseWriter) {
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

func (p *Passthru) Close() error {
	return nil
}
