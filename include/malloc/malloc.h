#ifndef _MALLOC_MALLOC_H
#define _MALLOC_MALLOC_H

#include <stddef.h>
#include <stdlib.h>

typedef struct _malloc_zone_t {
    void *reserved1;
    void *reserved2;
    size_t (*size)(struct _malloc_zone_t *zone, const void *ptr);
    void *(*malloc)(struct _malloc_zone_t *zone, size_t size);
    void *(*calloc)(struct _malloc_zone_t *zone, size_t num_items, size_t size);
    void *(*valloc)(struct _malloc_zone_t *zone, size_t size);
    void (*free)(struct _malloc_zone_t *zone, void *ptr);
    void *(*realloc)(struct _malloc_zone_t *zone, void *ptr, size_t size);
    void (*destroy)(struct _malloc_zone_t *zone);
    const char *zone_name;
} malloc_zone_t;

size_t malloc_size(const void *ptr);
size_t malloc_usable_size(const void *ptr);
malloc_zone_t *malloc_default_zone(void);
void *malloc_zone_malloc(malloc_zone_t *zone, size_t size);
void *malloc_zone_calloc(malloc_zone_t *zone, size_t num_items, size_t size);
void *malloc_zone_valloc(malloc_zone_t *zone, size_t size);
void *malloc_zone_realloc(malloc_zone_t *zone, void *ptr, size_t size);
void malloc_zone_free(malloc_zone_t *zone, void *ptr);

#endif /* _MALLOC_MALLOC_H */
