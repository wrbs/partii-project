SOURCES = $(wildcard *.dot)
BUILT = $(patsubst %.dot, %.tex, $(SOURCES))

.PHONY: all
all: $(BUILT)

%.tex: %.dot
	dot2tex --figonly $< > $@

.PHONY: clean
clean:
	rm *.tex
