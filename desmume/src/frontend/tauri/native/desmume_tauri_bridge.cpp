/*
	Copyright (C) 2026 DeSmuME team

	This file is free software: you can redistribute it and/or modify
	it under the terms of the GNU General Public License as published by
	the Free Software Foundation, either version 2 of the License, or
	(at your option) any later version.
*/

#include "desmume_tauri_bridge.h"

#include <string.h>

#include "../../interface/interface.h"

namespace
{
bool g_initialized = false;
bool g_rom_loaded = false;
bool g_paused = true;
uint16_t g_key_mask = 0;

void fill_blank_frame(uint8_t *buffer)
{
	memset(buffer, 0, DESMUME_TAURI_FRAME_BYTES);
	for (size_t i = 0; i < DESMUME_TAURI_FRAME_WIDTH * DESMUME_TAURI_FRAME_HEIGHT; ++i)
	{
		buffer[(i * 4) + 3] = 0xff;
	}
}
}

DESMUME_TAURI_API int tauri_desmume_init(void)
{
	if (g_initialized)
	{
		return 0;
	}

	const int result = desmume_init();
	if (result != 0)
	{
		return result;
	}

	desmume_set_language(1);
	desmume_set_savetype(0);

	g_initialized = true;
	g_rom_loaded = false;
	g_paused = true;
	g_key_mask = 0;

	return 0;
}

DESMUME_TAURI_API void tauri_desmume_shutdown(void)
{
	if (!g_initialized)
	{
		return;
	}

	desmume_free();
	g_initialized = false;
	g_rom_loaded = false;
	g_paused = true;
	g_key_mask = 0;
}

DESMUME_TAURI_API int tauri_desmume_open_rom(const char *filename)
{
	if (filename == NULL)
	{
		return -1;
	}

	const int init_result = tauri_desmume_init();
	if (init_result != 0)
	{
		return init_result;
	}

	const int open_result = desmume_open(filename);
	if (open_result != 0)
	{
		g_rom_loaded = false;
		g_paused = true;
		return open_result;
	}

	g_rom_loaded = true;
	g_paused = false;
	desmume_resume();

	return 0;
}

DESMUME_TAURI_API void tauri_desmume_set_paused(int paused)
{
	g_paused = paused != 0;

	if (!g_initialized)
	{
		return;
	}

	if (g_paused)
	{
		desmume_pause();
	}
	else if (g_rom_loaded)
	{
		desmume_resume();
	}
}

DESMUME_TAURI_API int tauri_desmume_reset(void)
{
	if (!g_initialized || !g_rom_loaded)
	{
		return -1;
	}

	desmume_reset();
	g_paused = false;

	return 0;
}

DESMUME_TAURI_API void tauri_desmume_set_key_mask(uint16_t key_mask)
{
	g_key_mask = key_mask;
	if (g_initialized)
	{
		desmume_input_keypad_update(g_key_mask);
	}
}

DESMUME_TAURI_API int tauri_desmume_run_frame(void)
{
	if (!g_initialized || !g_rom_loaded)
	{
		return -1;
	}

	desmume_input_keypad_update(g_key_mask);
	if (!g_paused)
	{
		desmume_cycle(0);
	}

	return 0;
}

DESMUME_TAURI_API int tauri_desmume_get_frame_rgba(uint8_t *buffer, size_t buffer_len)
{
	if (buffer == NULL || buffer_len < DESMUME_TAURI_FRAME_BYTES)
	{
		return -1;
	}

	if (!g_initialized || !g_rom_loaded)
	{
		fill_blank_frame(buffer);
		return 0;
	}

	desmume_draw_raw_as_rgbx(buffer);
	for (size_t i = 0; i < DESMUME_TAURI_FRAME_WIDTH * DESMUME_TAURI_FRAME_HEIGHT; ++i)
	{
		buffer[(i * 4) + 3] = 0xff;
	}

	return 0;
}
