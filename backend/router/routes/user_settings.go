package routes

import (
	"collectanea/app"
	"collectanea/controllers"
	"collectanea/middlewares"
	"collectanea/utils"

	"github.com/gin-gonic/gin"
)

func UserSettings(app *app.App, router *gin.RouterGroup) {
	grp := router.Group("/user")
	ctrl := controllers.NewUserSettingsController(app)

	grp.GET("/settings", middlewares.Auth(), utils.WP(ctrl.GetSettings))
	grp.PUT("/settings", middlewares.Auth(), utils.WP(ctrl.SaveSettings))
}
