-- Keep established users' choices; fresh installations have not completed onboarding.
UPDATE settings
SET force_multiconnection_downloads = TRUE
WHERE onboarded = FALSE;
