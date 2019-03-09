SELECT * FROM (
    SELECT * FROM youtube_videos 
    ORDER BY id DESC 
    LIMIT 2
) 
ORDER BY id ASC 
LIMIT 1;