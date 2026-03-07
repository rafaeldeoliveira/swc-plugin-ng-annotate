// Case 1: export class with /* @ngInject */ before constructor
export class MainController {
    /* @ngInject */ constructor($scope, $uiRouter){
        this.$scope = $scope;
    }
    static $inject = [
        "$scope",
        "$uiRouter"
    ];
}
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
    static $inject = [
        "$scope"
    ];
}
// Case 4: export default with /* @ngInject */ before function name
export default /* @ngInject */ function SomeConfig($translateProvider, $translatePartialLoaderProvider) {
    $translatePartialLoaderProvider.addPart('SamplePart');
}
SomeConfig.$inject = [
    "$translateProvider",
    "$translatePartialLoaderProvider"
];
;
// Case 5: /* @ngInject */ on the line before export default function
/* @ngInject */ export default function OtherConfig($translatePartialLoaderProvider) {
    $translatePartialLoaderProvider.addPart('OtherPart');
}
OtherConfig.$inject = [
    "$translatePartialLoaderProvider"
];
// Case 6: export default class with /* @ngInject */ on the constructor
export default class SomeControllerClass {
    /* @ngInject */ constructor($log, $rootScope){
        this.$log = $log;
    }
    static $inject = [
        "$log",
        "$rootScope"
    ];
}
// Case 7: export default anonymous function (no name, must use array annotation)
/* @ngInject */ export default [
    "$translateProvider",
    "$translatePartialLoaderProvider",
    function($translateProvider, $translatePartialLoaderProvider) {
        $translatePartialLoaderProvider.addPart('Anonymous');
    }
];
