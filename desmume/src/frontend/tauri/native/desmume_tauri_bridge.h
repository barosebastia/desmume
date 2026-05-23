/*
	Copyright (C) 2026 DeSmuME team

	This file is free software: you can redistribute it and/or modify
	it under the terms of the GNU General Public License as published by
	the Free Software Foundation, either version 2 of the License, or
	(at your option) any later version.
*/

#ifndef DESMUME_TAURI_BRIDGE_H
#define DESMUME_TAURI_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
# ifdef WIN_EXPORT
#  define DESMUME_TAURI_API extern "C" __declspec(dllexport)
# else
#  define DESMUME_TAURI_API extern "C" __declspec(dllimport)
# endif
#else
# define DESMUME_TAURI_API extern "C"
#endif

#define DESMUME_TAURI_FRAME_WIDTH 256
#define DESMUME_TAURI_FRAME_HEIGHT 384
#define DESMUME_TAURI_FRAME_BYTES (DESMUME_TAURI_FRAME_WIDTH * DESMUME_TAURI_FRAME_HEIGHT * 4)

enum DesmumeTauriKeyMask
{
	DESMUME_TAURI_KEY_A      = 1 << 0,
	DESMUME_TAURI_KEY_B      = 1 << 1,
	DESMUME_TAURI_KEY_SELECT = 1 << 2,
	DESMUME_TAURI_KEY_START  = 1 << 3,
	DESMUME_TAURI_KEY_RIGHT  = 1 << 4,
	DESMUME_TAURI_KEY_LEFT   = 1 << 5,
	DESMUME_TAURI_KEY_UP     = 1 << 6,
	DESMUME_TAURI_KEY_DOWN   = 1 << 7,
	DESMUME_TAURI_KEY_R      = 1 << 8,
	DESMUME_TAURI_KEY_L      = 1 << 9,
	DESMUME_TAURI_KEY_X      = 1 << 10,
	DESMUME_TAURI_KEY_Y      = 1 << 11
};

DESMUME_TAURI_API int tauri_desmume_init(void);
DESMUME_TAURI_API void tauri_desmume_shutdown(void);
DESMUME_TAURI_API int tauri_desmume_open_rom(const char *filename);
DESMUME_TAURI_API void tauri_desmume_set_paused(int paused);
DESMUME_TAURI_API int tauri_desmume_reset(void);
DESMUME_TAURI_API void tauri_desmume_set_key_mask(uint16_t key_mask);
DESMUME_TAURI_API int tauri_desmume_run_frame(void);
DESMUME_TAURI_API int tauri_desmume_get_frame_rgba(uint8_t *buffer, size_t buffer_len);

#endif
