# Codemap — hotmuses
Generated: 2026-04-16 | Files: 361 | Languages: javascript, python, rust, typescript

## Service Topology
  admin-dashboard      →  admin                http: /admin/configs/delete, /admin/configs/list, /admin/configs/save, /admin/letspact/moderation/avatars/bulk-resolve, /admin/letspact/moderation/avatars/delete-image, /admin/letspact/moderation/avatars/list, /admin/letspact/moderation/avatars/resolve, /admin/letspact/moderation/bulk-resolve, /admin/letspact/moderation/community-avatars/bulk-resolve, /admin/letspact/moderation/community-avatars/delete-image, /admin/letspact/moderation/community-avatars/list, /admin/letspact/moderation/community-avatars/resolve, /admin/letspact/moderation/delete-image, /admin/letspact/moderation/list, /admin/letspact/moderation/resolve, /admin/letspact/reports/list, /admin/letspact/reports/pact-feed-items, /admin/letspact/reports/resolve, /admin/login
  client-app           →  pages                http: /pages/categories, /pages/get-protected
  client-app           →  users                http: /users/configure-settings, /users/delete-account, /users/login
  letspact-app         →  letspact             http: /auth/social, /community/board/create, /community/board/delete, /community/board/feed, /community/board/publish, /community/board/unpublish, /community/boards, /community/create, /community/delete, /community/feed, /community/get, /community/invite, /community/invite/respond, /community/join, /community/leave, /community/member/ban, /community/member/remove, /community/member/role, /community/member/unban, /community/mine, /community/ownership/transfer, /community/pact/create, /community/read, /community/search, /community/update, /feed/comment/add, /feed/comment/delete, /feed/comment/hide, /feed/comments/get, /feed/get, /feed/item/boards, /feed/item/delete, /feed/item/get, /feed/item/update, /feed/like/toggle, /feed/likers/get, /feed/other-user/get, /feed/public, /feed/visibility/update, /friend/invite/cancel, /friend/invite/respond, /friend/invite/send, /friend/remove, /friends/get, /letspact, /pact/create, /pact/get, /pact/invite, /pact/invite/respond, /pact/join, /pact/leave, /pact/shared/get, /profile-data/get, /profile/complete, /report/comment, /report/feed-item, /report/pact, /search, /search/all, /settings/get, /settings/update, /signup, /step/complete, /step/freestyle/create, /step/progress, /user/block, /user/blocked/list, /user/unblock
  locales              →  letspact             http: /settings/update
  notifications        →  users                http: /users/set-notification-token

## admin [rust]

### Public
POST /login
  file: service/src/services/admin/login.rs
  in:   body{AdminLoginRequest}
GET /tracking/install
  file: service/src/services/admin/tracking.rs

## configs [rust]

### [auth: with_admin_auth]
POST /delete
  file: service/src/services/admin/configs/handlers.rs
  in:   body{DeleteConfigRequest}
  sql:  writes segmented_configs  [service/src/services/admin/configs/repository.rs:342]
POST /list
  file: service/src/services/admin/configs/handlers.rs
  in:   body{GetConfigsRequest}
  sql:  reads segmented_configs  [service/src/services/admin/configs/repository.rs:265]
POST /save
  file: service/src/services/admin/configs/handlers.rs
  in:   body{SaveConfigRequest}
  sql:  writes segmented_configs  [service/src/services/admin/configs/repository.rs:200]

## image-moderator [python]

### Public
GET /
  file: data/image-moderator/app.py:703
POST /hash/block
  file: data/image-moderator/app.py:866
GET /health
  file: data/image-moderator/app.py:878
POST /moderate
  file: data/image-moderator/app.py:709
POST /moderate/batch
  file: data/image-moderator/app.py:789
POST /moderate/url
  file: data/image-moderator/app.py:747

## letspact [rust]

### Public
POST /auth/social
  file: service/src/services/letspact/handlers/users.rs
  in:   body{SocialLoginRequest}, headers{x-timezone}
  sql:  writes lp_users  [service/src/services/letspact/user_repository.rs:161]
POST /feed/public
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{GetPublicFeedRequest}
POST /login
  file: service/src/services/letspact/handlers/users.rs
  in:   body{LoginRequest}, headers{x-timezone}
  sql:  writes lp_users  [service/src/services/letspact/user_repository.rs:382]
POST /signup
  file: service/src/services/letspact/handlers/users.rs
  in:   body{SignupRequest}, headers{x-timezone}
  sql:  writes lp_users  [service/src/services/letspact/user_repository.rs:382]

### [auth: with_admin_auth]
POST /letspact/moderation/avatars/bulk-resolve
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{BulkResolveAvatarModerationReviewRequest}
  sql:  writes lp_users  [service/src/services/letspact/moderation_service.rs:302]
POST /letspact/moderation/avatars/delete-image
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{DeleteAvatarModerationImageRequest}
  sql:  writes lp_users  [service/src/services/letspact/moderation_service.rs:346]
POST /letspact/moderation/avatars/list
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{GetAvatarModerationReviewsRequest}
POST /letspact/moderation/avatars/resolve
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{ResolveAvatarModerationReviewRequest}
  sql:  writes lp_users  [service/src/services/letspact/moderation_service.rs:272]
POST /letspact/moderation/bulk-resolve
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{BulkResolveModerationReviewRequest}
  sql:  writes feed_items  [service/src/services/letspact/moderation_service.rs:200]
POST /letspact/moderation/community-avatars/bulk-resolve
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{BulkResolveCommunityAvatarModerationReviewRequest}
  sql:  writes communities  [service/src/services/letspact/moderation_service.rs:448]
POST /letspact/moderation/community-avatars/delete-image
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{DeleteCommunityAvatarModerationImageRequest}
  sql:  writes communities  [service/src/services/letspact/moderation_service.rs:492]
POST /letspact/moderation/community-avatars/list
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{GetCommunityAvatarModerationReviewsRequest}
POST /letspact/moderation/community-avatars/resolve
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{ResolveCommunityAvatarModerationReviewRequest}
  sql:  writes communities  [service/src/services/letspact/moderation_service.rs:418]
POST /letspact/moderation/delete-image
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{DeleteModerationImageRequest}
  sql:  writes feed_items  [service/src/services/letspact/moderation_service.rs:538]
POST /letspact/moderation/list
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{GetModerationReviewsRequest}
POST /letspact/moderation/resolve
  file: service/src/services/letspact/handlers/moderation.rs
  in:   body{ResolveModerationReviewRequest}
  sql:  writes feed_items  [service/src/services/letspact/moderation_service.rs:170]
POST /letspact/reports/list
  file: service/src/services/letspact/handlers/reports.rs
  in:   body{GetReportsRequest}
POST /letspact/reports/pact-feed-items
  file: service/src/services/letspact/handlers/reports.rs
  in:   body{GetReportPactFeedItemsRequest}
POST /letspact/reports/resolve
  file: service/src/services/letspact/handlers/reports.rs
  in:   body{ResolveReportRequest}
  sql:  writes feed_items, lp_comments, lp_reports, pacts  [service/src/services/letspact/report_repository.rs:290, service/src/services/letspact/report_repository.rs:302, service/src/services/letspact/report_repository.rs:332, service/src/services/letspact/report_repository.rs:385]

### [auth: with_lp_auth]
POST /account/delete
  file: service/src/services/letspact/handlers/account.rs
  in:   body{DeleteAccountRequest}
POST /community/board/create
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{CreateCommunityBoardRequest}
POST /community/board/delete
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{DeleteCommunityBoardRequest}
  sql:  writes board_feed_items, community_boards  [service/src/services/letspact/community_board_repository.rs:75, service/src/services/letspact/community_board_repository.rs:81]
POST /community/board/feed
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{GetBoardFeedRequest}
POST /community/board/publish
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{PublishToBoardRequest}
  sql:  writes board_feed_items  [service/src/services/letspact/community_board_repository.rs:206]
POST /community/board/unpublish
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{RemoveFromBoardRequest}
  sql:  writes board_feed_items  [service/src/services/letspact/community_board_repository.rs:260]
POST /community/boards
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{GetCommunityBoardsRequest}
POST /community/create
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{CreateCommunityRequest}, body{GetCommunityRequest}
  sql:  writes community_members  [service/src/services/letspact/community_repository.rs:75]
POST /community/delete
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{DeleteCommunityRequest}
  sql:  writes communities, community_members, community_pacts  [service/src/services/letspact/community_repository.rs:798, service/src/services/letspact/community_repository.rs:804, service/src/services/letspact/community_repository.rs:810]
POST /community/feed
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{GetCommunityFeedRequest}
POST /community/get
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{GetCommunityRequest}
POST /community/invite
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{InviteToCommunityRequest}
POST /community/invite/respond
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{RespondToCommunityInviteRequest}
POST /community/join
  file: service/src/services/letspact/handlers/communities.rs
POST /community/leave
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{LeaveCommunityRequest}
  sql:  writes communities, community_members, community_pacts, feed_items, pact_members, pacts, steps  [service/src/services/letspact/community_repository.rs:1033, service/src/services/letspact/community_repository.rs:540, service/src/services/letspact/community_repository.rs:547, service/src/services/letspact/pact_repository.rs:583, service/src/services/letspact/pact_repository.rs:584, service/src/services/letspact/pact_repository.rs:586, service/src/services/letspact/pact_repository.rs:587]
POST /community/member/ban
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{BanCommunityMemberRequest}
  sql:  writes communities, community_members, community_pacts, feed_items, pact_members, pacts, steps  [service/src/services/letspact/community_repository.rs:1033, service/src/services/letspact/community_repository.rs:1186, service/src/services/letspact/community_repository.rs:1199, service/src/services/letspact/pact_repository.rs:583, service/src/services/letspact/pact_repository.rs:584, service/src/services/letspact/pact_repository.rs:586, service/src/services/letspact/pact_repository.rs:587]
POST /community/member/banned
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{GetBannedMembersRequest}
POST /community/member/remove
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{RemoveCommunityMemberRequest}
  sql:  writes communities, community_pacts, feed_items, pact_members, pacts, steps  [service/src/services/letspact/community_repository.rs:1033, service/src/services/letspact/community_repository.rs:678, service/src/services/letspact/pact_repository.rs:583, service/src/services/letspact/pact_repository.rs:584, service/src/services/letspact/pact_repository.rs:586, service/src/services/letspact/pact_repository.rs:587]
POST /community/member/role
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{UpdateMemberRoleRequest}
  sql:  writes community_members  [service/src/services/letspact/community_repository.rs:705]
POST /community/member/unban
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{UnbanCommunityMemberRequest}
  sql:  writes community_members  [service/src/services/letspact/community_repository.rs:1217]
POST /community/mine
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{GetMyCommunitiesRequest}
POST /community/ownership/transfer
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{TransferOwnershipRequest}
  sql:  writes community_members  [service/src/services/letspact/community_repository.rs:1308]
POST /community/pact/create
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{CreateCommunityPactRequest}
  sql:  writes community_pacts  [service/src/services/letspact/community_repository.rs:840]
POST /community/read
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{MarkCommunityReadRequest}
  sql:  writes community_members  [service/src/services/letspact/community_repository.rs:1238]
POST /community/search
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{SearchCommunitiesRequest}
POST /community/update
  file: service/src/services/letspact/handlers/communities.rs
  in:   body{GetCommunityRequest}, body{UpdateCommunityRequest}
POST /feed/comment/add
  file: service/src/services/letspact/handlers/engagement.rs
  in:   body{AddCommentRequest}
POST /feed/comment/delete
  file: service/src/services/letspact/handlers/engagement.rs
  in:   body{DeleteCommentRequest}
  sql:  writes lp_comments  [service/src/services/letspact/engagement_repository.rs:372]
POST /feed/comment/hide
  file: service/src/services/letspact/handlers/engagement.rs
  in:   body{HideCommentRequest}
  sql:  writes lp_comments  [service/src/services/letspact/engagement_repository.rs:445]
POST /feed/comments/get
  file: service/src/services/letspact/handlers/engagement.rs
  in:   body{GetCommentsRequest}
POST /feed/get
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{GetFeedRequest}
POST /feed/item/boards
  file: service/src/services/letspact/handlers/community_boards.rs
  in:   body{GetFeedItemBoardsRequest}
POST /feed/item/delete
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{DeleteFeedItemRequest}
  sql:  writes feed_items  [service/src/services/letspact/feed_repository.rs:284]
POST /feed/item/get
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{GetFeedItemRequest}
POST /feed/item/update
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{UpdateFeedItemRequest}
  sql:  writes feed_items, steps  [service/src/services/letspact/feed_repository.rs:413, service/src/services/letspact/feed_repository.rs:433]
POST /feed/like/toggle
  file: service/src/services/letspact/handlers/engagement.rs
  in:   body{ToggleLikeRequest}
  sql:  writes lp_likes  [service/src/services/letspact/engagement_repository.rs:135]
POST /feed/likers/get
  file: service/src/services/letspact/handlers/engagement.rs
  in:   body{GetLikersRequest}
POST /feed/other-user/get
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{GetUserFeedRequest}
POST /feed/visibility/update
  file: service/src/services/letspact/handlers/feed.rs
  in:   body{UpdateFeedItemVisibilityRequest}
  sql:  writes feed_items  [service/src/services/letspact/feed_repository.rs:455]
POST /friend/invite/cancel
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{CancelFriendInviteRequest}
  sql:  writes lp_friend_invites  [service/src/services/letspact/friendship_repository.rs:376]
POST /friend/invite/respond
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{RespondToFriendInviteRequest}
POST /friend/invite/send
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{SendFriendInviteRequest}
POST /friend/remove
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{RemoveFriendRequest}
  sql:  writes lp_friendships  [service/src/services/letspact/friendship_repository.rs:452]
POST /friends/get
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{GetFriendsRequest}
POST /pact/create
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{CreatePactRequest}
POST /pact/get
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{GetPactRequest}
POST /pact/invite
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{InviteToPactRequest}
POST /pact/invite/respond
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{RespondToPactInviteRequest}
  sql:  writes pact_members  [service/src/services/letspact/pact_repository.rs:422]
POST /pact/join
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{JoinPactRequest}
  sql:  writes pact_members  [service/src/services/letspact/pact_repository.rs:725]
POST /pact/leave
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{LeavePactRequest}
  sql:  writes pact_members  [service/src/services/letspact/pact_repository.rs:546]
POST /pact/shared/get
  file: service/src/services/letspact/handlers/pacts.rs
  in:   body{GetSharedPactRequest}
POST /profile-data/get
  file: service/src/services/letspact/handlers/users.rs
  in:   body{GetProfileRequest}, headers{x-timezone}
POST /profile/complete
  file: service/src/services/letspact/handlers/users.rs
  in:   body{CompleteProfileRequest}
  sql:  writes lp_users  [service/src/services/letspact/user_repository.rs:209]
POST /report/comment
  file: service/src/services/letspact/handlers/reports.rs
  in:   body{ReportCommentRequest}
  sql:  writes lp_reports  [service/src/services/letspact/report_repository.rs:180]
POST /report/feed-item
  file: service/src/services/letspact/handlers/reports.rs
  in:   body{ReportFeedItemRequest}
  sql:  writes lp_reports  [service/src/services/letspact/report_repository.rs:91]
POST /report/pact
  file: service/src/services/letspact/handlers/reports.rs
  in:   body{ReportPactRequest}
  sql:  writes lp_reports  [service/src/services/letspact/report_repository.rs:135]
POST /search
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{SearchUsersRequest}
POST /search/all
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{SearchRequest}
POST /settings/get
  file: service/src/services/letspact/handlers/users.rs
  in:   body{GetUserSettingsRequest}
POST /settings/set-notification-token
  file: service/src/services/letspact/handlers/users.rs
  in:   body{SetNotificationTokenRequest}
POST /settings/update
  file: service/src/services/letspact/handlers/users.rs
  in:   body{UpdateSettingsRequest}
  sql:  writes lp_users  [service/src/services/letspact/user_repository.rs:226]
POST /step/complete
  file: service/src/services/letspact/handlers/steps.rs
  in:   body{CompleteStepRequest}
POST /step/freestyle/create
  file: service/src/services/letspact/handlers/steps.rs
  in:   body{CreateFreestyleStepRequest}
POST /step/progress
  file: service/src/services/letspact/handlers/steps.rs
  in:   body{UpdateStepProgressRequest}
  sql:  writes steps  [service/src/services/letspact/step_repository.rs:799]
POST /user/block
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{BlockUserRequest}
  sql:  writes lp_friend_invites, lp_friendships, lp_user_blocks, pact_members  [service/src/services/letspact/block_repository.rs:104, service/src/services/letspact/block_repository.rs:61, service/src/services/letspact/block_repository.rs:81, service/src/services/letspact/block_repository.rs:90]
POST /user/blocked/list
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{GetBlockedUsersRequest}
POST /user/unblock
  file: service/src/services/letspact/handlers/friends.rs
  in:   body{UnblockUserRequest}
  sql:  writes lp_user_blocks  [service/src/services/letspact/block_repository.rs:137]

### Background
lp_auth_middleware
  file: service/src/services/letspact/authentication.rs
  sql:  writes lp_users  [service/src/services/letspact/user_repository.rs:382]

## pages [rust]

### Public
GET /fact
  file: service/src/services/pages/handlers.rs
GET /fact-image
  file: service/src/services/pages/handlers.rs

### [auth: with_auth]
POST /categories
  file: service/src/services/pages/handlers.rs
  in:   body{GetCategoriesRequest}
POST /get-protected
  file: service/src/services/pages/handlers.rs
  in:   body{GetRequest}

## service [rust]

### Public
GET /.well-known/apple-app-site-association
  file: service/src/main.rs
GET /.well-known/assetlinks.json
  file: service/src/main.rs
GET /health
  file: service/src/main.rs

### Background
main
  file: service/build.rs
main
  file: service/src/bin/load_tester.rs
main
  file: service/src/bin/send_lp_daily_reminders.rs
main
  file: service/src/bin/migrate.rs
main
  file: service/src/bin/send_pages_notifications.rs
  sql:  reads users
  sql:  writes users
main
  file: service/src/main.rs

## users [rust]

### Public
POST /login
  file: service/src/services/users/handlers.rs
  in:   headers{authorization}

### [auth: with_auth]
POST /configure-settings
  file: service/src/services/users/handlers.rs
  in:   body{ConfigureUserSettingsRequest}
  sql:  writes users  [service/src/services/users/user_repository.rs:164]

### [auth: with_lp_auth]
POST /account/delete
  file: service/src/services/users/handlers.rs
  in:   body{DeleteAccountRequest}
POST /settings/set-notification-token
  file: service/src/services/users/handlers.rs
  in:   body{SetNotificationTokenRequest}

