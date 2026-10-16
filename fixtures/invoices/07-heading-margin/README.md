# 07 — heading margin

Two-page Czech invoice whose first element in `<body>` is an `<h1 class="mt-8">`, followed by a 40-row items table.

Exercises:
- the heading's 32 px top margin collapses through `<body>` (CSS 2.1 §8.3.1), so page 1 starts 32 px lower than the
  content box; the paginator has to subtract that from the room page 1 has (F29). Before F29 a 22nd row was placed
  at 812 pt on a content box that ends at 799 pt and hung into the bottom margin; now it opens page 2, as in Chrome
- the table header repeats on page 2 (F12)
- the margin is not repeated on page 2, which starts at the top of the content box
- page 1 still holds one row more than Chrome's (21 vs 20): row heights drift by a fraction of a pixel and add up
  (`compare.py` threshold in `THRESHOLDS`)
