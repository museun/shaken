SELECT * FROM (
    SELECT * FROM local_songs 
    ORDER BY id DESC 
    LIMIT 2
) 
ORDER BY id ASC 
LIMIT 1;