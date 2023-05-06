.PHONE: all
all: countdown.bin

%.bin: %.o
	ld -m elf_i386 --oformat binary -N -e _start -o $@ $<

%.o: %.S
	as -32 $< -o $@

.PHONY: clean
clean:
	rm -f *.o *.bin
