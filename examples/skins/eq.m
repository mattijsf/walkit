// SPDX-License-Identifier: GPL-3.0-or-later
// The modern example skin's equalizer: a click on +12, 0 or -12 sets all ten bands to it, as
// the labels do in a classic skin. Compiled with mc.exe from the Wasabi SDK into eq.maki,
// which make-skins.py puts in the skin as scripts/eq.maki:
//
//     mc.exe eq.m

#include <lib/std.mi>

Function setBands(int value);

Global Button bandsMax, bandsFlat, bandsMin;

System.onScriptLoaded() {
	Group eq = getScriptGroup();
	bandsMax = eq.findObject("eq.max");
	bandsFlat = eq.findObject("eq.flat");
	bandsMin = eq.findObject("eq.min");
}

setBands(int value) {
	int band = 0;
	while (band < 10) {
		setEqBand(band, value);
		band++;
	}
}

bandsMax.onLeftClick() {
	setBands(127);
}

bandsFlat.onLeftClick() {
	setBands(0);
}

bandsMin.onLeftClick() {
	setBands(-127);
}
