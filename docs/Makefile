build_dir := build

proposal_src := proposal.tex
proposal_built := $(build_dir)/proposal.pdf
proposal_out := wnr21-ProjectProposal-sbh-nk.pdf

dissertation_src := dissertation.tex bib.bib $(wildcard dis_src/*.tex)
dissertation_built := $(build_dir)/dissertation.pdf
dissertation_out := wnr21-dissertation.pdf

LATEXMK := latexmk -pdf -pdflatex=lualatex -output-directory="$(build_dir)"

.PHONY: all
all: $(dissertation_out) $(proposal_out)

$(proposal_out): $(proposal_built)
	cp $^ $@
 
$(proposal_built): $(proposal_src)
	$(LATEXMK) $<

$(dissertation_out): $(dissertation_built)
	cp $^ $@

$(dissertation_built): $(dissertation_src)
	mkdir -p $(build_dir)/dis_src
	$(LATEXMK) $<

.PHONY: clean
clean:
	rm -rf $(build_dir)