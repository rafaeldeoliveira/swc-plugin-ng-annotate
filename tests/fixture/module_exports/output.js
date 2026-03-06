// Case 1: export class with /* @ngInject */ before constructor
export class MainController {
    /* @ngInject */ constructor($scope, $uiRouter){
        this.$scope = $scope;
    }
}
MainController.$inject = [
    "$scope",
    "$uiRouter"
];
// Case 2: export with /* @ngInject */ between export and function
export /* @ngInject */ function OtherController($scope, $uiRouter) {
    this.$scope = $scope;
}
OtherController.$inject = [
    "$scope",
    "$uiRouter"
];
// Case 3: export class with /** @ngInject */ on the class (already tested, sanity check)
/** @ngInject */ export class SanityController {
    constructor($scope){}
}
SanityController.$inject = [
    "$scope"
];
