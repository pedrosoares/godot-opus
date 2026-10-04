## OpusEncoder / OpusDecoder from GDScript. Run through tests/run.sh.
extends SceneTree

var failures := 0


func check(condition: bool, what: String) -> void:
	print("%s %s" % ["ok  " if condition else "FAIL", what])
	if not condition:
		failures += 1


func _initialize() -> void:
	var encoder := OpusEncoder.new()
	var decoder := OpusDecoder.new()
	check(not encoder.setup(44100, 1, 24000, true), "rejects 44.1 kHz")
	check(encoder.setup(16000, 1, 24000, true), "encoder setup")
	check(decoder.setup(16000, 1), "decoder setup")
	check(encoder.get_frame_size() == 320, "20 ms frames at 16 kHz")

	var bytes := 0
	var energy := 0.0
	var t := 0
	for f in 50:
		var frame := PackedFloat32Array()
		frame.resize(320)
		for i in 320:
			frame[i] = 0.5 * sin(TAU * 440.0 * t / 16000.0)
			t += 1
		var packet := encoder.encode(frame)
		bytes += packet.size()
		var pcm := decoder.decode(packet)
		if pcm.size() != 320:
			check(false, "frame %d decodes to 320 samples" % f)
		if f > 5:
			for s in pcm:
				energy += s * s
	check(bytes > 2000 and bytes < 4500, "about 24 kbit/s (%d bytes/s)" % bytes)
	var level := energy / (44 * 320)
	check(level > 0.06 and level < 0.2, "tone survives (level %.3f)" % level)
	check(decoder.decode_lost(PackedByteArray()).size() == 320, "concealment frame")
	check(encoder.encode(PackedFloat32Array([0.0])).is_empty(), "wrong frame size is refused")
	print("ALL PASSED" if failures == 0 else "FAILED: %d" % failures)
	quit(1 if failures else 0)
