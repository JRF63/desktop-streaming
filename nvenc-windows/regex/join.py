import subprocess

with open('nalus\\input.h264', 'wb') as out:
    with open('nalus\\csd.bin', 'rb') as f:
        out.write(f.read())
    for i in range(120):
        with open(f'nalus\\{i}.h264', 'rb') as f:
            out.write(f.read())

subprocess.call(['ffmpeg', '-i', 'nalus\\input.h264', '-c', 'copy', 'nalus\\output.mp4'])