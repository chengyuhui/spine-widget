#include <stddef.h>
#include <spine/spine.h>

/*
 * Internal API available for extension:
 */

void* _malloc (size_t size, const char* file, int line);
void* _calloc (size_t num, size_t size, const char* file, int line);
void _free (void* ptr);

void _setMalloc (void* (*_malloc) (size_t size));
void _setDebugMalloc (void* (*_malloc) (size_t size, const char* file, int line));
void _setFree (void (*_free) (void* ptr));

char* _readFile (const char* path, int* length);